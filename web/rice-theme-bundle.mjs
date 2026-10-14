import {validateWindowDecorations, validateThemeTokens, TYPOGRAPHY_SCHEMA, CONTROL_METRICS_SCHEMA, validateControlColors, validateSurfaceSkins,validateThemeIcons} from './rice-theme-customization.mjs';
import {validateCustomTerminals} from './rice-recipe-terminal-install.mjs';
import {validateThemePalettes} from './rice-theme-palettes.mjs';
import { paintCyberpunkCurtain } from './rice-cyberpunk-style.mjs';
import { paintMatrixCurtain, createMatrixMorph, paintMatrixMorph, matrixDOMSource } from './rice-matrix-style.mjs';
// Small, locally served Noto Sans SC subset for the signal-console captions (OFL).
export const signalFontReady = typeof FontFace !== 'undefined' && typeof document !== 'undefined'
    ? new FontFace('RiceWM Signal', `url(${new URL('./assets/fonts/ricewm-signal-cjk.ttf',import.meta.url).href})`).load().then(font=>{document.fonts.add(font);}).catch(()=>{})
    : Promise.resolve();
import {widgetSurfacePreferences} from './rice-widget-surface-preferences.mjs';
import {validateSpatialTheme} from './spatial/theme.mjs';
import {wallpaperFlight,wallpaperMedia} from './rice-wallpaper-scenes.mjs';
/** Portable theme configuration. Assets are references; personal data is never exported. */
const copy = value => structuredClone(value);
const modes = new Set(['Instant', 'Fade', 'Slide', 'Fold', 'Spring', 'Curtain']);
const events = new Set(['window.open', 'window.close', 'window.layout', 'menu.open', 'menu.close', 'menu.fold', 'hover', 'notification.open', 'notification.close', 'deck.open', 'deck.close', 'deck.switch']);
const fields = new Set(['opacity', 'x', 'y', 'scaleX', 'scaleY', 'reveal']);
const uuid = /^[A-Za-z0-9._@+-]{1,128}$/;
const text = (value, max = 256) => {
    if (typeof value !== 'string' || value.length > max || /[\x00-\x1f]/.test(value)) throw new TypeError('Invalid theme text');
    return value;
};
const number = (value, min, max) => {
    if (!Number.isFinite(value) || value < min || value > max) throw new TypeError(`Theme value must be between ${min} and ${max}`);
    return value;
};
export function validateThemeBundle(input = {}) {
    if (!input || typeof input !== 'object' || Array.isArray(input)) throw new TypeError('Invalid theme bundle');
    if (!Array.isArray(input.deck ?? [])) throw new TypeError('Quick menu sections must be a list');
    if (input.version !== undefined && input.version !== 1) throw new TypeError('Unsupported theme bundle version');
    if(input.$schema==='ricewm.theme/v2'){const {$schema,name,appearance,...fields}=input;input=fields;}
    const bundle = { ...copy(input), version: 1, wallpapers: [], extensions: [], motion: {}, layout: {}, deck: [] };
    if(input.spatial!==undefined)bundle.spatial=validateSpatialTheme(input.spatial);
    if (input.glass !== undefined && input.glass?.model !== 'hyprglass') throw new TypeError('Unsupported glass model');
    if(input.tiling !== undefined && (!input.tiling || typeof input.tiling.enabled!=='boolean'))throw new TypeError('Theme tiling needs an enabled flag');
    if(input.tiling?.arrangement!==undefined&&!['Split','Master stack','Scrolling'].includes(input.tiling.arrangement))throw new TypeError('Invalid theme tiling arrangement');
    if(input.tiling?.preserveCurrent !== undefined && typeof input.tiling.preserveCurrent !== 'boolean')throw new TypeError('Theme tiling preserveCurrent must be boolean');
    if(input.windowDecorations!==undefined)bundle.windowDecorations=validateWindowDecorations(input.windowDecorations);
    if(input.typography!==undefined)bundle.typography=validateThemeTokens(input.typography,TYPOGRAPHY_SCHEMA,'typography');
    if(input.metrics!==undefined)bundle.metrics=validateThemeTokens(input.metrics,CONTROL_METRICS_SCHEMA,'metrics');
    if(input.controlColors!==undefined)bundle.controlColors=validateControlColors(input.controlColors);
    if(input.surfaceSkins!==undefined)bundle.surfaceSkins=validateSurfaceSkins(input.surfaceSkins);
    if(input.icons!==undefined)bundle.icons=validateThemeIcons(input.icons);
    if (input.surface !== undefined) {
        const surface = input.surface;
        if (surface?.variant !== undefined && surface.variant !== 'cyberpunk') throw new TypeError('Unknown surface variant');
        if (!surface || !['plain', 'bracket', 'matrix'].includes(surface.frame) || !/^#[0-9a-f]{6}$/i.test(surface.edge || '')) throw new TypeError('Surface needs a frame style and hex edge color');
        if (surface.titleInk !== undefined && !/^#[0-9a-f]{6}$/i.test(surface.titleInk)) throw new TypeError('Surface title ink must be a hex color');
        bundle.surface = {...copy(surface), cornerLength:number(surface.cornerLength ?? 8, 3, 24), inset:number(surface.inset ?? 3, 1, 8)};
    }
    if (input.quickMenu !== undefined) {
        const menu = input.quickMenu;
        if (!menu || typeof menu !== 'object' || Array.isArray(menu)) throw new TypeError('Quick menu appearance must be an object');
        if (!['grid', 'list'].includes(menu.applications ?? 'grid')) throw new TypeError('Quick menu applications must use grid or list');
        if (!['comfortable', 'compact'].includes(menu.density ?? 'comfortable')) throw new TypeError('Quick menu density must be comfortable or compact');
        if (!['system', 'monospace'].includes(menu.font ?? 'system')) throw new TypeError('Quick menu font must be system or monospace');
        bundle.quickMenu = {...copy(menu), applications:menu.applications ?? 'grid', density:menu.density ?? 'comfortable', font:menu.font ?? 'system', width:number(menu.width ?? 850, 480, 1200)};
    }
    if (!Array.isArray(input.wallpapers ?? []) || (input.wallpapers?.length || 0) > 32) throw new TypeError('A theme supports up to 32 wallpapers');
    const seen = new Set();
    for (const item of input.wallpapers || []) {
        const id = text(item.id, 64);
        if (!/^[a-zA-Z0-9_-]+$/.test(id) || seen.has(id)) throw new TypeError('Wallpaper IDs must be unique');
        seen.add(id);
        const source = text(item.source, 2048);
        if (source.startsWith('/')) {
            if (source.split('/').some(part => part === '..' || part === '.') || source.startsWith('//')) throw new TypeError('Invalid wallpaper path');
        } else {
            const url = new URL(source);
            if (url.protocol !== 'https:' || url.username || url.password) throw new TypeError('Wallpaper downloads require an HTTPS URL');
        }
        if (item.type !== undefined && !['image','video'].includes(item.type)) throw new TypeError('Wallpaper type must be image or video');
        bundle.wallpapers.push({...wallpaperMedia(item),flight:wallpaperFlight(item.flight||{}),id,name:text(item.name||id,96),source,credit:text(item.credit||'',360)});
    }
    if(input.appViewsEnabled!==undefined&&typeof input.appViewsEnabled!=='boolean')throw new TypeError('App views must be on or off');
    validateThemePalettes(bundle);
    if (input.defaultWallpaper !== undefined) {
        if (!seen.has(input.defaultWallpaper)) throw new TypeError('Default wallpaper is missing');
        bundle.defaultWallpaper = input.defaultWallpaper;
    }
    if (!Array.isArray(input.extensions ?? []) || (input.extensions?.length || 0) > 32) throw new TypeError('A theme supports up to 32 extensions');
    seen.clear();
    for (const item of input.extensions || []) {
        if (!uuid.test(item.uuid) || seen.has(item.uuid)) throw new TypeError('Invalid or duplicate extension UUID');
        seen.add(item.uuid);
        const settings = copy(item.settings || {});
        if (!settings || Array.isArray(settings) || typeof settings !== 'object') throw new TypeError('Extension settings must be an object');
        if (JSON.stringify(settings).length > 16384) throw new TypeError('Extension preferences are too large');
        bundle.extensions.push({ uuid: item.uuid, required: item.required === true, settings, ...(item.replaceSettings===true?{replaceSettings:true}:{}) });
    }
    if(input.windowPlacement!==undefined){
        if(!Array.isArray(input.windowPlacement)||input.windowPlacement.length>128)throw new TypeError('Invalid window placement list');
        const seen=new Set();
        for(const item of input.windowPlacement){
            const p=item?.placement,identity=JSON.stringify([item?.key,item?.occurrence]);
            if(typeof item?.key!=='string'||item.key.length>2048||!Number.isInteger(item.occurrence)||item.occurrence<0||item.occurrence>127||seen.has(identity)||!p||!Number.isInteger(p.workspace)||p.workspace<0||p.workspace>3)throw new TypeError('Invalid window placement');
            seen.add(identity);
            for(const frame of [p.frame,p.restoreFrame].filter(Boolean))if(!['x','y','width','height'].every(k=>Number.isFinite(frame[k])&&Math.abs(frame[k])<=65535&&(!['width','height'].includes(k)||frame[k]>0)))throw new TypeError('Invalid window size');
        }
        bundle.windowPlacement=copy(input.windowPlacement);
    }
    if(input.terminalSetup!==undefined){
        const tools=input.terminalSetup?.tools;
        const custom=validateCustomTerminals(input.terminalSetup?.custom);
        if(!Array.isArray(tools)||tools.length>18||new Set(tools).size!==tools.length||tools.some(id=>!['fastfetch','btop','htop','cava','yazi','mc'].includes(id)&&!custom.some(entry=>entry.id===id)))throw new TypeError('Invalid terminal setup');
        bundle.terminalSetup={tools:[...tools],...(custom.length?{custom}:{})};
    }
    if (input.widgets !== undefined) {
        if (!Array.isArray(input.widgets) || input.widgets.length > 32) throw new TypeError('A theme supports up to 32 widget presets');
        const keys=new Set();
        bundle.widgets=input.widgets.map(item=>{
            if(!item || typeof item.key!=='string' || typeof item.package!=='string' || !uuid.test(item.key) || keys.has(item.key) || !uuid.test(item.package))throw new TypeError('Invalid theme widget identity');
            keys.add(item.key);
            if(item.required!==undefined&&typeof item.required!=='boolean')throw new TypeError('Invalid required widget flag');
            if(item.workspace!==undefined && item.workspace!=='all' && !(Number.isInteger(item.workspace)&&item.workspace>=0&&item.workspace<4))throw new TypeError('Invalid widget workspace');
            if(item.placement!==undefined&&item.placement!=='bottom-full-width')throw new TypeError('Invalid widget placement mode');
            if(item.rect && !['x','y','width','height'].every(k=>Number.isFinite(item.rect[k])&&Math.abs(item.rect[k])<=65535&&(!['width','height'].includes(k)||item.rect[k]>=48)))throw new TypeError('Invalid widget placement');
            if(item.settings && (typeof item.settings!=='object'||Array.isArray(item.settings)||JSON.stringify(item.settings).length>16384))throw new TypeError('Invalid widget settings');
            widgetSurfacePreferences(item.surface);
            return copy(item);
        });
    }
    for (let [event, animation] of Object.entries(input.motion || {})) {
        if (animation?.preset === 'Ayanami') animation = { ...animation, preset: 'Curtain' };
        if (!events.has(event) || !animation || !modes.has(animation.preset)) throw new TypeError('Unsupported motion event or preset');
        const normalized = { ...copy(animation), preset: animation.preset, duration: number(animation.duration ?? 180, 0, 2000) };
        if (animation.easing !== undefined && !['linear', 'ease-out', 'ease-in-out', 'out-expo'].includes(animation.easing)) throw new TypeError(`Unsupported easing for ${event}`);
        if (animation.keyframes !== undefined) {
            if (!Array.isArray(animation.keyframes) || animation.keyframes.length < 2 || animation.keyframes.length > 16) throw new TypeError('Animations need 2–16 keyframes');
            let previous = -1;
            normalized.keyframes = animation.keyframes.map(frame => {
                const at = number(frame.at, 0, 1);
                if (at <= previous) throw new TypeError('Animation times must increase');
                previous = at;
                const result = { at };
                for (const [key, value] of Object.entries(frame)) {
                    if (key === 'at') continue;
                    if (!fields.has(key)) throw new TypeError('Unsupported animation property');
                    result[key] = number(value, key === 'x' || key === 'y' ? -256 : 0, key === 'x' || key === 'y' ? 256 : key.startsWith('scale') ? 2 : 1);
                }
                return result;
            });
            if (normalized.keyframes[0].at !== 0 || normalized.keyframes.at(-1).at !== 1) throw new TypeError('Animations must start at 0 and end at 1');
        }
        bundle.motion[event] = normalized;
    }
    for (const [key, value] of Object.entries(input.layout || {})) {
        if (!['dock.style', 'dock.position', 'dock.top_bar.visible', 'dock.top_bar.transparent', 'appearance.animations', 'appearance.animations.elastic'].includes(key)) throw new TypeError(`Unsupported theme layout setting: ${key}`);
        bundle.layout[key] = text(String(value), 64);
    }
    const sections = new Set(['dashboard', 'applications', 'workspaces', 'controls', 'notifications', 'wallpapers', 'notes', 'clipboard']);
    for (const section of input.deck || []) {
        if (!sections.has(section) || bundle.deck.includes(section)) throw new TypeError('Invalid quick menu section');
        bundle.deck.push(section);
    }
    for (const settings of [input.settings, input.overrides]) {
        if (settings !== undefined && (!settings || typeof settings !== 'object' || Array.isArray(settings) || Object.entries(settings).some(([key, value]) => !/^[a-z][a-z0-9_.-]{0,127}$/.test(key) || typeof value !== 'string' || value.length > 16384))) throw new TypeError('Theme settings must be a map of setting names to values');
    }
    if (input.background) themeBackgroundPreferences(input.background);
    return bundle;
}

/** Separate cacheable asset preparation from activation and selection writes.
 * prepareAssets returns a detached candidate. It may populate the asset cache,
 * but cannot invoke this adapter's setup or selection callbacks. Callers still
 * need to choose which bundle contributions to apply to the desktop.
 */
export function createThemePreparation({assets, activate = async () => {}, persist = async () => {}}) {
    if (![assets, activate, persist].every(callback => typeof callback === 'function')) {
        throw new TypeError('Theme preparation requires function adapters');
    }
    const prepareAssets = async (source, options = {}) => {
        if (!source || typeof source !== 'object' || Array.isArray(source)) throw new TypeError('Invalid theme');
        const candidate = copy(source);
        candidate.bundle = validateThemeBundle(candidate.bundle || {});
        await assets(candidate, options);
        candidate.bundle = validateThemeBundle(candidate.bundle || {});
        return candidate;
    };
    const prepare = async source => {
        const candidate = await prepareAssets(source);
        await activate(candidate);
        await persist(candidate);
        Object.assign(source, candidate);
    };
    Object.defineProperty(prepare, 'prepareAssets', {value: prepareAssets});
    return prepare;
}

export function sampleThemeMotion(animation, progress, reduced = false) {
    let t = reduced ? 1 : Math.min(1, Math.max(0, progress));
    const identity = { opacity: 1, x: 0, y: 0, scaleX: 1, scaleY: 1, reveal: 1 };
    if (!animation || animation.preset === 'Instant' || reduced) return identity;
    if (animation.easing === 'ease-out') t = 1 - (1-t)**3;
    if (animation.easing === 'ease-in-out') t = t*t*(3-2*t);
    if (animation.easing === 'out-expo') t = t === 1 ? 1 : 1-2**(-10*t);
    if (animation.keyframes) {
        const frames = animation.keyframes.map((frame,index,all) => Object.assign({}, identity, ...all.slice(0,index+1)));
        const end = frames.findIndex(frame => frame.at >= t);
        const right = frames[end < 0 ? frames.length - 1 : end];
        const left = frames[Math.max(0, end - 1)];
        const fraction = right.at === left.at ? 1 : (t - left.at) / (right.at - left.at);
        return Object.fromEntries(Object.entries(identity).map(([key, fallback]) => [key, (left[key] ?? fallback) + ((right[key] ?? fallback) - (left[key] ?? fallback)) * fraction]));
    }
    const ease = 1 - (1 - t) ** 3;
    if (animation.preset === 'Fade') return { ...identity, opacity: ease };
    if (animation.preset === 'Slide') return { ...identity, opacity: ease, x: 24 * (1 - ease) };
    if (animation.preset === 'Spring') { const spring = t === 1 ? 1 : 1-Math.exp(-7.5*t)*Math.cos(10.5*t); return { ...identity, opacity: ease, scaleX: .9 + .1 * spring, scaleY: .9 + .1 * spring }; }
    if (animation.preset === 'Curtain') {
        const frame = Math.min(1,t/.56), reveal = Math.max(0,(t-.56)/.44);
        return { ...identity, scaleX: .025 + .975*(frame === 1 ? 1 : 1-2**(-10*frame)), reveal: reveal === 1 ? 1 : 1-2**(-10*reveal) };
    }
    return { ...identity, scaleX: .025 + .975 * ease, scaleY: .08 + .92 * ease, reveal: Math.max(0, (t - .65) / .35) };
}

/** Import/export the same portable document users edit in RicePaper. */
export function importTheme(source) {
    if (typeof source !== 'string' || new TextEncoder().encode(source).length > 65536) throw new TypeError('Theme document exceeds 64 KiB');
    const document = JSON.parse(source);
    if (document.$schema !== 'ricewm.theme/v2' || !document.appearance || typeof document.appearance !== 'object' || Array.isArray(document.appearance)) throw new TypeError('Unsupported theme document');
    text(document.name, 48);
    if (Object.values(document.appearance).some(value => typeof value !== 'string')) throw new TypeError('Appearance values must be strings');
    validateThemeBundle(document);
    return document;
}
export function exportTheme(document) { return JSON.stringify(importTheme(JSON.stringify(document)), null, 2) + '\n'; }

/** Recipe media is limited to installed artwork and host-prepared cache files.
 * A pathname is not an atomic resource handle; providers must still enforce
 * identity when opening it. Arbitrary user files need a separate import step. */
export function assertRecipeThemeAssetPath(path) {
    if(typeof path!=='string'||!(
        /^\/usr\/share\/ricewm\/wallpapers\/[a-z0-9-]+\.(png|jpe?g|webp|svg|gif|mp4|webm)$/.test(path)||
        /^\/root\/\.cache\/ricewm\/theme-assets\/[a-f0-9]{64}\.(png|jpe?g|webp|svg|gif|mp4|webm)$/.test(path)
    ))throw Error('Recipe theme media must be prepared local assets');
}

async function assertUnaliasedRecipeAsset(filesystem,path) {
    const resolve=filesystem?.resolvePathAsync||filesystem?.resolvePath;
    if(typeof resolve!=='function'||await resolve.call(filesystem,path)!==path){
        throw Error('Recipe theme asset filesystem identity cannot be verified');
    }
}

/** Download media through RiceDataProvider, caching by source URL in the desktop VFS. */
export class ThemeAssetStore {
    async verifyVideo(source,path) {
        if(!/\.mp4(?:[?#]|$)/i.test(source)&&!path.endsWith('.mp4'))return path;
        const bytes=new Uint8Array(await this.fs.mergedFileBytesAsync(path)||[]);
        if(!bytes.length||bytes.length>16*1024*1024)throw Error('MP4 wallpaper must be at most 16 MiB');
        const expected=/^https:\/\/account\.env\.now\/v1\/wallpapers\/wp-[a-z0-9-]+\/(?:video|preview)\/([a-f0-9]{64})\.mp4$/.exec(source)?.[1];
        if(expected){const actual=[...new Uint8Array(await crypto.subtle.digest('SHA-256',Uint8Array.from(bytes).buffer))].map(byte=>byte.toString(16).padStart(2,'0')).join('');if(actual!==expected)throw Error('Video wallpaper integrity check failed');}
        return path;
    }
    constructor({ filesystem, dataProvider, fetchMedia, limit = 128 * 1024 * 1024 }) {
        this.fetchMedia = fetchMedia; this.fs = filesystem; this.provider = dataProvider; this.limit = limit; this.pending = new Map();
    }
    async resolve(source, {allowDownload = true} = {}) {
        if (typeof source !== 'string' || !source) throw new TypeError('A background source is required');
        if (source.startsWith('/') && !source.startsWith('//')) {
            if(!allowDownload){assertRecipeThemeAssetPath(source);await assertUnaliasedRecipeAsset(this.fs,source);}
            if (source.split('/').some(part => part === '.' || part === '..')) throw new TypeError('Invalid background path');
            if (!this.fs.mergedKind(source)) throw new Error('Background file is unavailable');
            return this.verifyVideo(source,source);
        }
        const url = new URL(source);
        if (url.protocol !== 'https:' || url.username || url.password) throw new TypeError('Background downloads require HTTPS');
        // A recipe cannot join an in-flight download started by another caller.
        if(!allowDownload)return this.download(source,{allowDownload:false});
        if (this.pending.has(source)) return this.pending.get(source);
        const task = this.download(source); this.pending.set(source, task);
        try { return await task; } finally { this.pending.delete(source); }
    }
    async download(source,{allowDownload=true}={}) {
        const digest = [...new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(source)))].map(byte => byte.toString(16).padStart(2, '0')).join('');
        const root = '/root/.cache/ricewm/theme-assets';
        const metadata = `${root}/${digest}.json`;
        if(!allowDownload)await assertUnaliasedRecipeAsset(this.fs,metadata);
        const cached = await this.fs.mergedFileBytesAsync(metadata);
        if (cached) {
            const record = JSON.parse(new TextDecoder().decode(new Uint8Array(cached)));
            const prefix=root+'/'+digest+'.';
            if(record.source!==source||typeof record.path!=='string'||!record.path.startsWith(prefix)||!/^(png|jpe?g|webp|svg|gif|mp4|webm)$/i.test(record.path.slice(prefix.length)))throw new Error('Invalid theme asset cache record');
            if(!allowDownload){assertRecipeThemeAssetPath(record.path);await assertUnaliasedRecipeAsset(this.fs,record.path);}
            if(this.fs.mergedKind(record.path)===1)return this.verifyVideo(source,record.path);
        }
        if(!allowDownload)throw new Error('Theme asset is not prepared. Download it separately before using this recipe.');
        if (!this.fetchMedia && !this.provider?.hasHttp) throw new Error('This background is not downloaded; connect to download it');
        // Theme media is an explicit desktop download, separate from guest app traffic.
        // The browser transport observes CORS and never includes credentials.
        const response = this.fetchMedia
            ? await this.fetchMedia(source, { credentials: 'omit', mode: 'cors', signal: AbortSignal.timeout(30000) })
            : await this.provider.fetchHttp(source, { cache: 'no-cache' });
        if (!response.ok) throw new Error(`Background download failed (${response.status})`);
        const video=/\.mp4(?:[?#]|$)/i.test(source)||String(response.headers?.get?.('content-type')||'').split(';')[0]==='video/mp4';
        const mediaLimit=video?Math.min(this.limit,16*1024*1024):this.limit;
        const declared = Number(response.headers?.get?.('content-length'));
        if (declared > mediaLimit) throw new Error('Background exceeds the download limit');
        let bytes;
        if(response.body?.getReader){
            const reader=response.body.getReader(),chunks=[];let length=0;
            try{for(;;){const {value,done}=await reader.read();if(done)break;length+=value.byteLength;if(length>mediaLimit)throw new Error('Background exceeds the download limit');chunks.push(value);}}
            catch(error){await reader.cancel().catch(()=>{});throw error;}
            finally{reader.releaseLock();}
            bytes=new Uint8Array(length);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.byteLength;}
        }else bytes = new Uint8Array(await response.arrayBuffer());
        if (!bytes.length || bytes.length > mediaLimit) throw new Error('Background exceeds the download limit');
        const mime = String(response.headers?.get?.('content-type') || '').split(';')[0];
        const extensions = { 'image/png': 'png', 'image/jpeg': 'jpg', 'image/webp': 'webp', 'image/svg+xml': 'svg', 'image/gif': 'gif', 'video/mp4': 'mp4', 'video/webm': 'webm' };
        const extension = extensions[mime] || new URL(source).pathname.match(/\.(png|jpe?g|webp|svg|gif|mp4|webm)$/i)?.[1];
        if (!extension) throw new Error('Unsupported background media type');
        let directory = ''; for (const part of root.split('/').filter(Boolean)) { directory += `/${part}`; if (!this.fs.mergedKind(directory)) this.fs.mkdir(directory, 0o700); }
        const path = `${root}/${digest}.${extension}`;
        this.fs.putFile(path, bytes, 0o600);
        this.fs.putFile(metadata, new TextEncoder().encode(JSON.stringify({ source, path, mime, bytes: bytes.length })), 0o600);
        await this.fs.flush(); return this.verifyVideo(source,path);
    }
}

export function themeBackgroundPreferences(background = {}) {
    const color = value => typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value);
    const mode = ['Crop', 'Fit', 'Stretch', 'Tile', 'Center'].includes(background.mode) ? background.mode : 'Crop';
    switch (background.type) {
        case 'solid':
            if (!color(background.color)) throw new TypeError('A solid background needs a hex color');
            return { 'appearance.background.type': 'Solid', 'appearance.background.color': background.color };
        case 'gradient': {
            const values = { 'appearance.background.type': 'Gradient' };
            for (const key of ['start', 'middle', 'end', 'accent']) {
                if (!color(background[key])) throw new TypeError(`Gradient ${key} needs a hex color`);
                values[`appearance.background.gradient.${key}`] = background[key];
            }
            return values;
        }
        case 'image': return { 'appearance.background.type': 'Image', 'appearance.wallpaper.path': text(background.source, 2048), 'appearance.wallpaper.mode': mode };
        case 'video': return { 'appearance.background.video.url': text(background.source, 2048), 'appearance.background.type': 'Video' };
        default: throw new TypeError('Choose an image, video, gradient, or solid background');
    }
}

/** Paint a clipped signal curtain without squeezing its lettering during the wipe. */
export function paintSignalCurtain(context, frame, reveal, accent = '#cc1515', surface = null) {
    if(surface?.variant === 'cyberpunk') return paintCyberpunkCurtain(context,frame,reveal,accent);
    const {x, y, width, height} = frame;
    context.save();
    context.beginPath(); context.rect(x, y, width * (1 - reveal), height); context.clip();
    context.fillStyle = accent; context.fillRect(x, y, width, height);
    context.translate(x, y); context.strokeStyle = '#130707'; context.fillStyle = '#130707'; context.lineWidth = 1;
    for (const [cx,cy,sx,sy] of [[7,7,1,1],[width-7,7,-1,1],[7,height-7,1,-1],[width-7,height-7,-1,-1]]) {
        context.beginPath(); context.moveTo(cx,cy+sy*10);context.lineTo(cx,cy);context.lineTo(cx+sx*10,cy);context.stroke();
    }
    context.textAlign='center'; context.textBaseline='middle';
    context.font=`${Math.min(24, Math.max(12,width/22))}px monospace`;
    context.fillText('A Y A N A M I',width/2,height/2,Math.max(1,width-36));
    if (width > 180 && height > 110) {
        context.textAlign='right';context.font='9px monospace, "RiceWM Signal"';
        context.fillText('綾波',width-18,24);
        context.fillText('TYPE 00',width-18,40);
        context.textAlign='left';context.fillText('AY // 000',18,height-20);
        for(let i=0;i<25;i++)context.fillRect(18+i*3,height-48,i%3===0?2:1,12-(i%4));
        context.fillRect(width-58,height-18,38,2);
    }
    context.restore();
}

/** Shared sampled motion for DOM shell surfaces; canvas uses the same sampler. */
export function animateThemeElement(element, definition, { closing = false, reduced = false, baseTransform = '', clip = true, signal = false, matrix = false, surface = null, overlayZIndex = 100 } = {}) {
    if (!element?.animate || !definition || reduced || definition.preset === 'Instant' || !definition.duration) return Promise.resolve();
    const curtain = definition.preset === 'Curtain' && !definition.keyframes ? element.ownerDocument.createElement('div') : null;
    const previousPosition = element.style.position;
    let matrixCanvas, matrixModel, cyberCanvas, matrixFrame = 0;
    const cyber=surface?.variant==='cyberpunk';
    if(matrix && curtain)matrixModel=createMatrixMorph(matrixDOMSource(element));
    if (curtain) {
        if (getComputedStyle(element).position === 'static') element.style.position = 'relative';
        curtain.setAttribute('aria-hidden','true');
        curtain.style.cssText = 'position:absolute;inset:0;z-index:100;pointer-events:none;background:var(--deck-accent,var(--theme-motion-accent,#cc1515));transform-origin:left;border-radius:inherit';
        if (signal || matrix || cyber) {
            const canvas=element.ownerDocument.createElement('canvas');
            canvas.width=Math.max(1,Math.round(element.clientWidth));canvas.height=Math.max(1,Math.round(element.clientHeight));
            canvas.style.cssText='width:100%;height:100%;display:block';
            const accent=getComputedStyle(element).getPropertyValue('--deck-accent').trim() || getComputedStyle(element).getPropertyValue('--theme-motion-accent').trim() || '#cc1515';
            if(matrix)paintMatrixCurtain(canvas.getContext('2d'),{x:0,y:0,width:canvas.width,height:canvas.height},0,accent);
            else paintSignalCurtain(canvas.getContext('2d'),{x:0,y:0,width:canvas.width,height:canvas.height},0,accent,surface);
            curtain.append(canvas);
            if (matrix) { matrixCanvas=canvas; curtain.style.background='transparent'; }
            if(cyber){cyberCanvas=canvas;curtain.style.background='transparent';}
        }
        if(matrix) {
            const bounds=element.getBoundingClientRect();
            curtain.style.cssText=`position:fixed;left:${bounds.left}px;top:${bounds.top}px;width:${bounds.width}px;height:${bounds.height}px;z-index:100;pointer-events:none;background:transparent`;
            curtain.style.zIndex = String(overlayZIndex);
            element.parentNode.append(curtain);
        } else element.append(curtain);
    }
    const frames = Array.from({ length: 33 }, (_, index) => {
        const motion = sampleThemeMotion(definition, closing ? 1 - index / 32 : index / 32);
        const t=closing ? 1-index/32 : index/32;
        const settle=Math.max(0,Math.min(1,(t-.64)/.34));
        return { opacity: matrix && curtain ? settle*settle*(3-2*settle) : motion.opacity, transform: `${baseTransform} translate(${motion.x}px, ${motion.y}px) scale(${matrix ? 1 : motion.scaleX}, ${motion.scaleY})`,
            ...(clip && !curtain ? { clipPath: `inset(0 ${(1 - motion.reveal) * 100}% 0 0)` } : {}) };
    });
    if (curtain && !matrix && !cyber) curtain.animate(Array.from({length:33},(_,index)=>({clipPath:`inset(0 ${sampleThemeMotion(definition,closing?1-index/32:index/32).reveal*100}% 0 0)`})), {duration:definition.duration,easing:'linear',fill:'forwards'});
    const animation = element.animate(frames, { duration: definition.duration, easing: 'linear' });
    if (cyberCanvas) {
        const paint=()=>{
            const progress=Math.min(1,Number(animation.currentTime||0)/definition.duration);
            const reveal=sampleThemeMotion(definition,closing?1-progress:progress).reveal;
            const context=cyberCanvas.getContext("2d");context.clearRect(0,0,cyberCanvas.width,cyberCanvas.height);
            paintCyberpunkCurtain(context,{x:0,y:0,width:cyberCanvas.width,height:cyberCanvas.height},reveal,surface.edge);
            if(animation.playState==='running')matrixFrame=requestAnimationFrame(paint);
        };paint();
    }
    if (matrixCanvas) {
        const paint = () => {
            const progress = Math.min(1, Number(animation.currentTime || 0) / definition.duration);
            const t = closing ? 1-progress : progress;
            const context=matrixCanvas.getContext('2d');
            context.clearRect(0,0,matrixCanvas.width,matrixCanvas.height);
            paintMatrixMorph(context, {x:0,y:0,width:matrixCanvas.width,height:matrixCanvas.height}, t, matrixModel, {particlesOnly:true});
            if (animation.playState === 'running') matrixFrame=requestAnimationFrame(paint);
        };
        paint();
    }
    return animation.finished.catch(() => {}).finally(()=>{cancelAnimationFrame(matrixFrame);curtain?.remove();if(curtain)element.style.position=previousPosition;});
}
