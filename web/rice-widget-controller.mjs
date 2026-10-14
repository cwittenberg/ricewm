import {t} from './rice-i18n.mjs';
import {widgetSurfacePreferences} from './rice-widget-surface-preferences.mjs';
import {plasmaSettings} from './rice-plasma-package.mjs';
/** Persistent desktop widget instances. Providers own content; Rice owns placement. */
export const WIDGET_PATH = '/root/.local/share/ricewm/widgets/state.json';
const clone = value => structuredClone(value);
const ID = /^[a-zA-Z0-9][a-zA-Z0-9._:@+-]{0,127}$/;
const validId = value => typeof value === 'string' && ID.test(value);
const MAX_INSTANCES = 64;
const MAX_BYTES = 512 * 1024;
const color = value => typeof value === 'string' && /^#[a-f0-9]{6}([a-f0-9]{2})?$/i.test(value);
const finite = (value, min, max) => Number.isFinite(value) && value >= min && value <= max;
export const RICE_WIDGET_PACKAGES = Object.freeze([
    { id: 'rice.clock', name: 'Clock', description: 'Time and date on your workspace.', size: [320, 160], nodes: [{type:'clock'}] },
    { id: 'rice.spectrum', name: 'Audio spectrum', description: 'Live Rice Player audio. Waiting for audio when silent.', size: [480, 140], nodes: [{type:'spectrum'}] },
    { id: 'rice.text', name: 'Desktop label', description: 'A label or reminder for your workspace.', size: [280, 120], nodes: [{type:'text', text:'Make yourself at home.'}] },
].map(item => Object.freeze({schema:'rice.widget/v1', ...item})));

export const GLAVA_WIDGET_PACKAGE=Object.freeze({schema:'glava.visualizer/v1',id:'org.glava.visualizer',name:'GLava',description:'Original GLava audio visualizer.',version:'6fc0e32da2c54954a3784c2ced1024e576ba8d8d',size:[800,240],fields:[{name:'frameRate',type:'Int',get label(){return t('Animation rate (fps)');},description:'30 reduces CPU use; 60 gives smoother motion.',default:30,min:15,max:60}],source:Object.freeze({provider:'runtime',id:'glava'})});
/** Resolve only sources supported by the shared widget installer. */
export function widgetInstallReference(source,packageId){
    if(source?.provider==='opendesktop'&&typeof source.id==='string'&&/^\d+$/.test(source.id))return 'plasma:'+source.id;
    if(source?.provider==='runtime'&&source.id==='glava'&&packageId===GLAVA_WIDGET_PACKAGE.id)return 'runtime:glava';
    return null;
}
export function validateWidgetPackage(input) {
    if(input?.schema==='glava.visualizer/v1'){
        if(input.id!==GLAVA_WIDGET_PACKAGE.id||input.version!==GLAVA_WIDGET_PACKAGE.version)throw Error('Unsupported GLava runtime');
        return clone(GLAVA_WIDGET_PACKAGE);
    }
    if(input?.schema==='plasma.applet/v1'){
        if(!validId(input.id)||typeof input.name!=='string'||!input.name.trim()||input.name.length>96||(typeof input.entry!=='string'||!input.entry.startsWith('contents/ui/')||!input.entry.endsWith('.qml')||input.entry.includes('\\')||input.entry.split('/').some(p=>!p||p==='.'||p==='..')||!Array.isArray(input.files)||(!input.rootPackage&&!input.files.includes(input.entry)))||!Array.isArray(input.fields)||input.fields.length>256||!Array.isArray(input.files)||input.files.length>2048||!/^\/root\/\.local\/share\/ricewm\/widgets\/packages\/[a-f0-9]{64}$/.test(input.root)||input.files.some(p=>typeof p!=='string'||p.startsWith('/')||p.includes('\\')||p.split('/').some(s=>!s||s==='.'||s==='..')))throw Error('Invalid Plasma widget package');
        if(input.requiredPackages&&(!Array.isArray(input.requiredPackages)||input.requiredPackages.length>8||input.requiredPackages.some(id=>!validId(id)||id===input.id)))throw Error('Invalid widget dependencies');
        if(input.rootPackage&&(!validId(input.rootPackage)||input.rootPackage===input.id))throw Error('Invalid shared widget package');
        if(input.fileModes!==undefined&&(!input.fileModes||typeof input.fileModes!=='object'||Array.isArray(input.fileModes)||Object.entries(input.fileModes).some(([path,mode])=>!input.files.includes(path)||!Number.isInteger(mode)||mode<0||mode>0o777)))throw Error('Invalid widget file modes');
        if(!Array.isArray(input.size)||input.size.length!==2||!input.size.every(n=>finite(n,48,2048)))throw Error('Invalid widget size');
        return clone(input);
    }
    if (input?.schema !== 'rice.widget/v1' || !validId(input.id) || typeof input.name !== 'string' || !input.name.trim() || input.name.length > 96) throw Error('Invalid widget package');
    if (!Array.isArray(input.size) || input.size.length !== 2 || !input.size.every(n=>finite(n,48,2048))) throw Error('Invalid preferred widget size');
    if (!Array.isArray(input.nodes) || !input.nodes.length || input.nodes.length > 32) throw Error('A widget needs 1–32 content nodes');
    const nodes = input.nodes.map(node => {
        if (!['clock','text','spectrum'].includes(node.type)) throw Error('Unsupported widget content type');
        if (node.text !== undefined && (typeof node.text !== 'string' || node.text.length > 2048)) throw Error('Widget text is too long');
        if (node.color !== undefined && !color(node.color)) throw Error('Invalid widget color');
        if (node.size !== undefined && !finite(node.size,8,128)) throw Error('Invalid widget text size');
        return {type:node.type, ...(node.text === undefined ? {} : {text:node.text}), ...(node.color ? {color:node.color}:{}), ...(node.size ? {size:node.size}:{})};
    });
    return {schema:input.schema,id:input.id,name:input.name,description:String(input.description || '').slice(0,512),size:[...input.size],nodes};
}
function settings(input = {},pkg=null) {
    if(['plasma.applet/v1','glava.visualizer/v1'].includes(pkg?.schema))return plasmaSettings(input,pkg.fields);
    if (!input || typeof input !== 'object' || Array.isArray(input)) throw Error('Invalid widget settings');
    const result = {};
    for (const [key,value] of Object.entries(input)) {
        if (key==='text' && typeof value==='string' && value.length<=2048) result[key]=value;
        else if (key==='color' && color(value)) result[key]=value;
        else if (['glass','showDate','hour12'].includes(key) && typeof value==='boolean') result[key]=value;
        else if (key==='opacity' && finite(value,.1,1)) result[key]=value;
        else if (key==='style' && ['bars','radial','wave'].includes(value)) result[key]=value;
        else throw Error(`Unsupported widget setting: ${key}`);
    }
    return result;
}
export function widgetRect(rect, viewport = {width:1024,height:768}) {
    if (!rect || !['x','y','width','height'].every(k=>Number.isFinite(rect[k]))) throw Error('Invalid widget placement');
    const width = Math.min(Math.max(48,rect.width),Math.max(48,viewport.width));
    const height = Math.min(Math.max(48,rect.height),Math.max(48,viewport.height));
    return {x:Math.round(Math.max(0,Math.min(rect.x,viewport.width-width))),y:Math.round(Math.max(0,Math.min(rect.y,viewport.height-height))),width:Math.round(width),height:Math.round(height)};
}
function validateDocument(value) {
    if (value?.version!==1 || !Array.isArray(value.instances) || value.instances.length>MAX_INSTANCES || !Array.isArray(value.packages) || value.packages.length>128 || !Array.isArray(value.suppressed) || value.suppressed.length>256) throw Error('Invalid saved widget layout');
    const packages=value.packages.map(validateWidgetPackage), seen=new Set();
    for (const p of packages) { if (seen.has(p.id) || RICE_WIDGET_PACKAGES.some(b=>b.id===p.id)) throw Error('Duplicate widget package'); seen.add(p.id); }
    function validateDependencies(p,ancestors=new Set()){
        if(ancestors.has(p.id))throw Error('Widget dependency cycle');
        const next=new Set([...ancestors,p.id]);
        for(const id of new Set([...(p.requiredPackages||[]),...(p.rootPackage?[p.rootPackage]:[])])){
            const parent=packages.find(candidate=>candidate.id===id);
            if(!parent||parent.schema!=='plasma.applet/v1')throw Error('Missing shared widget package');
            validateDependencies(parent,next);
        }
    }
    for(const p of packages)validateDependencies(p);
    seen.clear();
    const appletIds=new Set();
    for (const i of value.instances) {
        if (!validId(i.id) || seen.has(i.id) || !validId(i.package) || !(i.workspace==='all'||Number.isInteger(i.workspace)&&i.workspace>=0&&i.workspace<4) || typeof i.enabled!=='boolean') throw Error('Invalid saved widget instance');
        seen.add(i.id);settings(i.settings,packages.find(p=>p.id===i.package));widgetSurfacePreferences(i.surface);
        if(i.placement!==undefined&&i.placement!=='bottom-full-width')throw Error('Invalid widget placement mode');
        if(i.appletId!==undefined){if(!Number.isInteger(i.appletId)||i.appletId<1||i.appletId>0xffffffff||appletIds.has(i.appletId))throw Error('Invalid widget applet identity');appletIds.add(i.appletId);}
        if (!i.owner || !['personal','theme'].includes(i.owner.kind) || (i.owner.kind==='theme' && (typeof i.owner.theme!=='string'||i.owner.theme.length>128||!validId(i.owner.key)))) throw Error('Invalid widget owner');
        if (!i.rect || !['x','y','width','height'].every(k=>finite(i.rect[k],k==='x'||k==='y'?-65535:48,65535))) throw Error('Invalid saved widget geometry');
    }
    if (value.suppressed.some(k=>typeof k!=='string'||k.length>512)) throw Error('Invalid suppressed widget slot');
    if (value.activeTheme !== null && (typeof value.activeTheme!=='string'||value.activeTheme.length>128)) throw Error('Invalid widget theme');
    if (typeof value.enabled!=='boolean') throw Error('Invalid widget switch');
    return clone(value);
}
export class RiceWidgetController {
    constructor({filesystem=null, changed=()=>{}, diagnostic=()=>{}, id=()=>globalThis.crypto.randomUUID()}={}) {
        this.runtimeLogs=new Map();this.runtimeErrors=new Map();this.filesystem=filesystem;this.changed=changed;this.diagnostic=diagnostic;this.makeId=id;
        this.state={version:1,enabled:true,activeTheme:null,instances:[],packages:[],suppressed:[]};
        this.workspace=0;this.viewport={width:1024,height:768};this.editing=false;this.selected=null;this.history=[];this.queue=Promise.resolve();this.closed=false;this.loadError=null;
        this.ready=this.load();
    }
    async load() {
        try {
            const bytes=await this.filesystem?.mergedFileBytesAsync?.(WIDGET_PATH) ?? this.filesystem?.mergedFileBytes?.(WIDGET_PATH);
            if(bytes?.length){
                if(bytes.length>MAX_BYTES)throw Error('Saved widget layout is too large');
                // Browser VFS views can use shared Wasm memory; TextDecoder requires ordinary memory.
                this.state=validateDocument(JSON.parse(new TextDecoder().decode(new Uint8Array(bytes))));
            }
        } catch(error){this.loadError=String(error.message||error);this.diagnostic({source:'widgets',state:'restore-failed',message:this.loadError});}
        this.publish();
    }
    catalog(){return [...RICE_WIDGET_PACKAGES,...this.state.packages].map(clone);}
    visible(i){return this.state.enabled && i.enabled && (i.workspace==='all'||i.workspace===this.workspace) && (i.owner.kind==='personal'||i.owner.theme===this.state.activeTheme);}
    snapshot(){return {version:1,enabled:this.state.enabled,workspace:this.workspace,activeTheme:this.state.activeTheme,editing:this.editing,selected:this.selected,canUndo:this.history.length>0,error:this.loadError,packages:this.catalog(),instances:this.state.instances.map(i=>({...clone(i),error:this.runtimeErrors.get(i.id)||null,visible:this.visible(i),rect:this.instanceRect(i)}))};}
    instanceRect(i){
        if(i.placement!=='bottom-full-width')return widgetRect(i.rect,this.viewport);
        const area=this.workArea||{x:0,y:0,...this.viewport};
        const height=Math.min(i.rect.height,area.height);
        return widgetRect({x:area.x,y:area.y+area.height-height,width:area.width,height},this.viewport);
    }
    publish(){if(!this.closed)this.changed(this.snapshot());}
    environment({workspace=this.workspace,width=this.viewport.width,height=this.viewport.height,workArea=this.workArea}={}) {
        const next={width:Math.max(48,width),height:Math.max(48,height)};
        const changed=workspace!==this.workspace||next.width!==this.viewport.width||next.height!==this.viewport.height||JSON.stringify(workArea)!==JSON.stringify(this.workArea);
        if(workspace!==this.workspace){this.editing=false;this.selected=null;}
        this.workspace=workspace;this.viewport=next;this.workArea=workArea;
        return changed;
    }
    async persist(value) {
        if(!this.filesystem)throw Error('Widget storage is unavailable');
        const bytes=new TextEncoder().encode(JSON.stringify(validateDocument(value)));
        if(bytes.length>MAX_BYTES)throw Error('Widget storage limit exceeded');
        for(const path of ['/root/.local/share/ricewm/widgets']) {
            if(this.filesystem.mergedKind?.(path)!==2)await this.filesystem.mkdir(path);
        }
        const old=this.filesystem.mergedFileBytes?.(WIDGET_PATH);
        try {await this.filesystem.putFile(WIDGET_PATH,bytes,0o600);await (this.filesystem.flush?.() ?? this.filesystem.flushOverlay?.());}
        catch(error){
            try {
                if(old)await this.filesystem.putFile(WIDGET_PATH,old,0o600);
                else await this.filesystem.unlink?.(WIDGET_PATH);
                await (this.filesystem.flush?.() ?? this.filesystem.flushOverlay?.());
            } catch(recovery){this.diagnostic({source:'widgets',state:'storage-recovery-failed',message:String(recovery.message||recovery)});}
            throw error;
        }
    }
    run(action,data={},commit=null,isCurrent=null) {
        const operation=this.queue.catch(()=>{}).then(async()=>{
            await this.ready;if(this.closed)throw Error('Widgets are closed');
            const ensureBuiltin=action==='ensure-spatial-builtin',checkOwner=()=>{if(ensureBuiltin&&(this.closed||typeof isCurrent!=='function'||isCurrent()!==true))throw Error('Builtin spatial widget owner changed');};
            checkOwner();
            if(action==='state')return this.snapshot();
            if(action==='log'){
                const instance=this.state.instances.find(i=>i.id===data.id);if(!instance)return null;
                const lines=this.runtimeLogs.get(instance.package)||[];
                lines.push(new Date().toISOString()+' ['+instance.id+'] '+String(data.message||'').slice(0,4096));
                this.runtimeLogs.set(instance.package,lines.slice(-200));return null;
            }
            if(action==='status'){if(!this.state.instances.some(i=>i.id===data.id))return this.snapshot();if(data.error)this.runtimeErrors.set(data.id,String(data.error).slice(0,512));else this.runtimeErrors.delete(data.id);this.publish();return this.snapshot();}
            if(action==='files'){
                let p=this.catalog().find(p=>p.id===data.package);if(p?.schema!=='plasma.applet/v1')throw Error('Package content unavailable');
                const visited=new Set();while(p.rootPackage){if(visited.has(p.id))throw Error('Widget dependency cycle');visited.add(p.id);p=this.catalog().find(candidate=>candidate.id===p.rootPackage);if(!p)throw Error('Shared widget package unavailable');}
                return Promise.all(p.files.map(async path=>{const bytes=await this.filesystem?.mergedFileBytesAsync?.(p.root+'/'+path)??this.filesystem?.mergedFileBytes?.(p.root+'/'+path);if(!bytes)throw Error('Widget package file is missing: '+path);return {path,bytes:new Uint8Array(bytes)};}));
            }
            if(action==='edit'){
                const selected=this.state.instances.find(i=>i.id===data.id);
                if(data.id&&!selected)throw Error('Widget not found');
                this.editing=data.enabled!==false;this.selected=selected?.id||this.state.instances.find(i=>this.visible(i))?.id||null;this.publish();return this.snapshot();}
            if(this.loadError)throw Error(`Saved widget layout needs recovery: ${this.loadError}`);
            if(action==='identity'){
                const instance=this.state.instances.find(i=>i.id===data.id);if(!instance)throw Error('Widget not found');
                if(instance.appletId!==undefined)return instance.appletId;
                // Plasma exposes a uint applet ID. Allocate and persist it in
                // the same serialized store as the real desktop instance;
                // UUID truncation would permit collisions and shared caches.
                const used=new Set([this.state,...this.history].flatMap(state=>state.instances.map(i=>i.appletId)));
                let appletId=1;while(used.has(appletId))appletId++;
                const draft=clone(this.state);draft.instances.find(i=>i.id===data.id).appletId=appletId;
                await this.persist(draft);this.state=draft;
                for(const state of this.history){const previous=state.instances.find(i=>i.id===data.id);if(previous)previous.appletId=appletId;}
                return appletId;
            }
            const before=clone(this.state), draft=clone(this.state);
            const item=()=>{const found=draft.instances.find(i=>i.id===data.id);if(!found)throw Error('Widget not found');return found;};
            if(ensureBuiltin){
                // This engine-owned convenience is not a portable recipe grant.
                // Installed selectors and arbitrary provider IDs never install.
                const selection=data?.spatial;
                if(data?.theme!=='construct.theme'||!selection||selection.engine!=='rice-spatial'||selection.api!==1||selection.world!=='world.json'||Object.keys(data).some(k=>!['theme','spatial'].includes(k))||Object.keys(selection).some(k=>!['engine','api','world'].includes(k)))throw Error('Unsupported builtin spatial widget selection');
                if(draft.activeTheme!==data.theme)throw Error('Builtin widget theme is not active');
                if(draft.instances.some(i=>i.package===GLAVA_WIDGET_PACKAGE.id)||draft.suppressed.includes('construct.theme:glava'))return this.snapshot();
                if(draft.instances.length>=MAX_INSTANCES)throw Error('Maximum of 64 widget instances');
                if(!draft.packages.some(p=>p.id===GLAVA_WIDGET_PACKAGE.id))draft.packages.push(clone(GLAVA_WIDGET_PACKAGE));
                draft.instances.push({id:this.makeId(),package:GLAVA_WIDGET_PACKAGE.id,workspace:'all',enabled:true,owner:{kind:'theme',theme:data.theme,key:'glava',local:true},rect:widgetRect({x:32,y:64,width:800,height:240},this.viewport),settings:settings({},GLAVA_WIDGET_PACKAGE),surface:widgetSurfacePreferences({background:'none',border:false})});
            } else if(action==='add'){
                const p=this.catalog().find(p=>p.id===data.package);if(!p)throw Error('Widget package is unavailable');
                if(draft.instances.length>=MAX_INSTANCES)throw Error('Maximum of 64 widget instances');
                const rect=widgetRect(data.rect||{x:Math.max(32,(this.viewport.width-p.size[0])/2)+draft.instances.length%6*24,y:Math.max(64,(this.viewport.height-p.size[1])/3)+draft.instances.length%6*24,width:p.size[0],height:p.size[1]},this.viewport);
                draft.instances.push({id:this.makeId(),package:p.id,workspace:this.workspace,enabled:true,owner:{kind:'personal'},autoSize:p.schema==='plasma.applet/v1'&&!data.rect,rect,settings:settings(data.settings,p),surface:widgetSurfacePreferences(data.surface)});
            } else if(action==='order'){
                const i=item(),index=draft.instances.indexOf(i);
                if(!['front','back','forward','backward'].includes(data.direction))throw Error('Invalid widget order');
                draft.instances.splice(index,1);const at=data.direction==='front'?draft.instances.length:data.direction==='back'?0:data.direction==='forward'?Math.min(index+1,draft.instances.length):Math.max(0,index-1);draft.instances.splice(at,0,i);
            } else if(action==='update'||action==='measure'){
                const i=item();
                if(action==='measure'&&!i.autoSize)return this.snapshot();
                if(data.rect){i.rect=widgetRect(data.rect,this.viewport);i.autoSize=false;delete i.placement;}
                if(data.workspace!==undefined){if(!(data.workspace==='all'||Number.isInteger(data.workspace)&&data.workspace>=0&&data.workspace<4))throw Error('Workspace unavailable');i.workspace=data.workspace;}
                if(data.surface!==undefined){widgetSurfacePreferences(data.surface);i.surface=widgetSurfacePreferences({...i.surface,...data.surface});}
                if(data.settings)i.settings={...i.settings,...settings(data.settings,this.catalog().find(p=>p.id===i.package))};
                if(data.enabled!==undefined){if(typeof data.enabled!=='boolean')throw Error('Invalid widget switch');i.enabled=data.enabled;}
            } else if(action==='duplicate'){
                if(draft.instances.length>=MAX_INSTANCES)throw Error('Maximum of 64 widget instances');
                const i=clone(item());i.rect=this.instanceRect(i);delete i.placement;i.id=this.makeId();i.owner={kind:'personal'};delete i.recipe;delete i.appletId;i.rect=widgetRect({...i.rect,x:i.rect.x+24,y:i.rect.y+24},this.viewport);draft.instances.push(i);
            } else if(action==='remove'){
                const i=item();if(i.owner.kind==='theme')draft.suppressed.push(`${i.owner.theme}:${i.owner.key}`);
                draft.instances=draft.instances.filter(i=>i.id!==data.id);
            } else if(action==='use-theme'){
                if(!draft.activeTheme)throw Error('Select a theme first');const i=item();i.owner={kind:'theme',theme:draft.activeTheme,key:i.id,local:true};
            } else if(action==='keep'){
                const i=item();if(i.owner.kind==='theme')draft.suppressed.push(`${i.owner.theme}:${i.owner.key}`);i.owner={kind:'personal'};delete i.recipe;
            } else if(action==='enabled'){
                if(typeof data.enabled!=='boolean')throw Error('Invalid widget switch');draft.enabled=data.enabled;
            } else if(action==='install'||action==='install-packages'){
                const inputs=action==='install'?[data]:data.packages;
                if(!Array.isArray(inputs)||!inputs.length||inputs.length>8)throw Error('Invalid widget installation transaction');
                const staged=inputs.map(input=>({package:validateWidgetPackage(input.package),files:input.files}));
                if(staged.length>1&&typeof this.filesystem?.unlink!=='function')throw Error('Widget storage cannot roll back dependency installs');
                const identities=new Set();
                for(const input of staged){
                    const p=input.package;
                    if(identities.has(p.id))throw Error('Duplicate widget installation');identities.add(p.id);
                    if(RICE_WIDGET_PACKAGES.some(b=>b.id===p.id))throw Error('Built-in widget cannot be replaced');
                    if(p.schema==='plasma.applet/v1'){
                        if(!Array.isArray(input.files)||input.files.length!==p.files.length)throw Error('Widget files are missing');
                        let total=0;const seen=new Set();
                        for(const file of input.files){if(!p.files.includes(file.path)||seen.has(file.path)||!(file.bytes instanceof Uint8Array))throw Error('Unexpected widget file');seen.add(file.path);total+=file.bytes.length;}
                        if(total>32*1024*1024)throw Error('Widget files exceed 32 MiB');
                    }
                    const existing=draft.packages.findIndex(b=>b.id===p.id);if(existing>=0)draft.packages[existing]=p;else draft.packages.push(p);
                }
                for(const input of staged){
                    const existing=before.packages.find(p=>p.id===input.package.id);
                    if(existing&&existing.root!==input.package.root&&before.packages.some(p=>!identities.has(p.id)&&(p.rootPackage===existing.id||p.requiredPackages?.includes(existing.id))))throw Error('Update dependent widgets together with their shared package');
                }
                validateDocument(draft); // Validate the entire graph before any writes.
                const written=[];
                try {
                    for(const input of staged)if(input.package.schema==='plasma.applet/v1')for(const file of input.files){
                        const path=input.package.root+'/'+file.path;
                        const old=await this.filesystem.mergedFileBytesAsync?.(path)??this.filesystem.mergedFileBytes?.(path);
                        // Content-addressed package roots must be immutable.
                        if(old){if(old.length!==file.bytes.length||old.some((byte,index)=>byte!==file.bytes[index]))throw Error('Widget archive content hash collision');continue;}
                        await this.filesystem.mkdir(path.slice(0,path.lastIndexOf('/')));
                        written.push(path);await this.filesystem.putFile(path,new Uint8Array(file.bytes),0o600);
                    }
                    await this.persist(draft);
                    await commit?.();
                }catch(error){
                    for(const path of written.reverse())await this.filesystem.unlink?.(path);
                    await this.persist(before);
                    throw error;
                }
                this.history.push(before);if(this.history.length>24)this.history.shift();this.state=draft;this.publish();return this.snapshot();
            } else if(action==='uninstall'){
                if(RICE_WIDGET_PACKAGES.some(b=>b.id===data.package))throw Error('Built-in widget cannot be uninstalled');
                if(draft.instances.some(i=>i.package===data.package))throw Error('Remove this package’s widget instances before uninstalling');
                if(draft.packages.some(p=>p.rootPackage===data.package||p.requiredPackages?.includes(data.package)))throw Error('Another widget requires this shared package');
                draft.packages=draft.packages.filter(p=>p.id!==data.package);
            } else if(action==='undo'){
                if(!this.history.length)return this.snapshot();
                const previous=this.history.at(-1);await this.persist(previous);this.state=clone(previous);this.history.pop();this.publish();return this.snapshot();
            } else if(action==='recipe'){
                if(!validId(data.id)||!Array.isArray(data.widgets)||data.widgets.length>32)throw Error('Invalid widget recipe');
                const keys=new Set();
                for(const entry of data.widgets){
                    if(!validId(entry.key)||keys.has(entry.key))throw Error('Recipe widget keys must be unique');keys.add(entry.key);
                    const p=this.catalog().find(p=>p.id===entry.package);if(!p)throw Error('Recipe widget unavailable: '+entry.package);
                    const previous=draft.instances.find(i=>i.recipe?.id===data.id&&i.recipe.key===entry.key);
                    const id=previous?.id||this.makeId();
                    const instance={id,package:p.id,recipe:{id:data.id,key:entry.key},workspace:entry.workspace??data.workspace??this.workspace,enabled:entry.enabled!==false,owner:data.theme?{kind:'theme',theme:data.theme,key:id,local:true}:{kind:'personal'},rect:widgetRect(entry.rect||previous?.rect||{x:32,y:64,width:p.size[0],height:p.size[1]},this.viewport),settings:settings(entry.settings||{},p),surface:widgetSurfacePreferences(entry.surface??previous?.surface)};
                    if(previous){if(previous.appletId!==undefined)instance.appletId=previous.appletId;draft.instances[draft.instances.indexOf(previous)]=instance;}else draft.instances.push(instance);
                }
                draft.instances=draft.instances.filter(i=>i.recipe?.id!==data.id||keys.has(i.recipe.key));
                if(draft.instances.length>MAX_INSTANCES)throw Error('Maximum of 64 widget instances');
            } else if(action==='theme'){
                if(typeof data.theme!=='string'||data.theme.length>128||!Array.isArray(data.widgets)||data.widgets.length>32)throw Error('Invalid theme widget collection');
                const keys=new Set();
                for(const entry of data.widgets){
                    if(!validId(entry.key)||keys.has(entry.key))throw Error('Theme widget keys must be unique');keys.add(entry.key);
                    if(entry.package===GLAVA_WIDGET_PACKAGE.id&&!draft.packages.some(p=>p.id===entry.package))draft.packages.push(clone(GLAVA_WIDGET_PACKAGE));
                    const p=[...RICE_WIDGET_PACKAGES,...draft.packages].find(p=>p.id===entry.package);
                    if(entry.placement!==undefined&&entry.placement!=='bottom-full-width')throw Error('Invalid widget placement mode');
                    if(!p){if(entry.required)throw Error(`Required widget unavailable: ${entry.package}`);continue;}
                    if(draft.suppressed.includes(`${data.theme}:${entry.key}`))continue;
                    const existing=draft.instances.find(i=>(i.owner.kind==='theme'&&i.owner.theme===data.theme&&i.owner.key===entry.key)||(entry.replaceSettings===true&&i.id===entry.key&&i.package===entry.package));
                    if(existing){
                        if(entry.replaceSettings===true){existing.settings=settings(entry.settings,p);existing.surface=widgetSurfacePreferences(entry.surface);existing.owner={kind:'theme',theme:data.theme,key:entry.key};existing.enabled=true;}
                        continue;
                    }
                    draft.instances.push({id:this.makeId(),package:p.id,workspace:entry.workspace??this.workspace,enabled:true,owner:{kind:'theme',theme:data.theme,key:entry.key},...(entry.placement?{placement:entry.placement}:{}),rect:widgetRect(entry.rect||{x:32,y:64,width:p.size[0],height:p.size[1]},this.viewport),settings:settings(entry.settings,p),surface:widgetSurfacePreferences(entry.surface)});
                }
                // A removed preset stays dormant; local edits for remaining slots survive reapply.
                draft.instances=draft.instances.filter(i=>i.owner.kind!=='theme'||i.owner.theme!==data.theme||i.owner.local===true||keys.has(i.owner.key));
                draft.activeTheme=data.theme;
            } else throw Error(`Unknown widget action: ${action}`);
            checkOwner();
            if(JSON.stringify(draft)!==JSON.stringify(before))await this.persist(draft);
            try {checkOwner();await commit?.();checkOwner();} catch(error){await this.persist(before);throw error;}
            if(action==='theme')this.history=[];
            else if(action!=='measure'&&!ensureBuiltin){this.history.push(before);if(this.history.length>24)this.history.shift();}
            this.state=draft;
            if(action==='add'&&data.edit!==false){this.editing=true;this.selected=draft.instances.at(-1).id;}
            this.publish();return this.snapshot();
        });this.queue=operation.catch(()=>{});return operation;
    }
    async restore(state){await this.persist(state);this.state=clone(state);this.publish();}
    close(){this.closed=true;this.editing=false;}
}
