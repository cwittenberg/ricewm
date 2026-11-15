import {nativeClientRect} from '../rice-native-window-stack.mjs';

export const GNOME_QUICK_SETTINGS_TOPIC='org.ricewm.gnome/quick-settings';
export const GNOME_QUICK_SETTINGS_FRAME_PREFIX='676e6f6d6571737631';
const MOUNT_SCHEMA='rice.gnome-quick-settings-mount/v1';
const MAX_FRAME_BYTES=16*1024*1024;
const inRect=(rect,x,y)=>rect&&x>=rect.x&&y>=rect.y&&x<rect.x+rect.width&&y<rect.y+rect.height;
const intersect=(a,b)=>{const x=Math.max(a.x,b.x),y=Math.max(a.y,b.y);return {x,y,width:Math.max(0,Math.min(a.x+a.width,b.x+b.width)-x),height:Math.max(0,Math.min(a.y+a.height,b.y+b.height)-y)};};
const quickWindow=window=>window?.entrypoint==='rice-settings'&&window.arguments?.includes('--quick-controls')&&!window.arguments.includes('--session-menu');

export function readGnomeQuickSettingsFrames(frames=[],liveApps=null,previous=new Map(),canvasFactory=(width,height)=>new OffscreenCanvas(width,height)){
    if(!Array.isArray(frames)||frames.length>256)throw new RangeError('Quick Settings frame registry exceeds its bound');
    const registry=new Map();let bytes=0;
    for(const frame of frames){
        const identity=String(frame?.identity||''),app=String(frame?.app||'');
        const width=frame?.width??frame?.surface?.width,height=frame?.height??frame?.surface?.height;
        if(!new RegExp('^'+GNOME_QUICK_SETTINGS_FRAME_PREFIX+'[a-f0-9]{46}$').test(identity)||registry.has(identity)||!/^\d+$/.test(app)||BigInt(app)<1n||BigInt(app)>0xffffffffffffffffn||
            liveApps&&!liveApps.has(app)||!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||width>2048||height>2048)throw new TypeError('Invalid owned Quick Settings frame');
        bytes+=width*height*4;if(bytes>MAX_FRAME_BYTES)throw new RangeError('Quick Settings frame registry byte limit exceeded');
        let surface=frame.surface;
        if(surface){if(typeof surface.getContext!=='function')throw new TypeError('Invalid owned Quick Settings frame');}
        else{
            if(!(frame.pixels instanceof Uint32Array)||frame.pixels.length!==width*height||!Number.isSafeInteger(frame.revision)||frame.revision<1)throw new TypeError('Invalid owned Quick Settings pixels');
            const retained=previous.get(identity);
            if(retained?.app===app&&retained.revision===frame.revision&&retained.surface.width===width&&retained.surface.height===height)surface=retained.surface;
            else{
                surface=canvasFactory(width,height);const context=surface.getContext('2d'),rgba=context.createImageData(width,height);
                for(let i=0;i<frame.pixels.length;i++){const pixel=frame.pixels[i];rgba.data.set([(pixel>>>16)&255,(pixel>>>8)&255,pixel&255,pixel>>>24],i*4);}
                context.putImageData(rgba,0,0);
            }
        }
        registry.set(identity,{app,surface,revision:frame.revision});
    }
    return registry;
}

function checkedRecords(records){
    if(!Array.isArray(records)||records.length>16)throw new RangeError('Quick Settings extension surface limit exceeded');
    const json=JSON.stringify(records);
    if(new TextEncoder().encode(json).byteLength>512*1024)throw new RangeError('Quick Settings surface data exceeds its bound');
    const copies=JSON.parse(json),uuids=new Set();
    for(const record of copies){
        if(record.schema!=='rice.gnome-quick-settings-surface/v1'||typeof record.uuid!=='string'||!record.uuid||record.uuid.length>256||uuids.has(record.uuid))throw new TypeError('Invalid Quick Settings extension identity');
        uuids.add(record.uuid);
        if(typeof record.stylesheet!=='string'||record.stylesheet.length>65536)throw new TypeError('Invalid Quick Settings stylesheet');
        const ids=new Set();let nodes=0;
        const visit=(actor,depth)=>{
            if(!actor||typeof actor!=='object'||++nodes>1024||depth>24||!Array.isArray(actor.children)||!Number.isSafeInteger(actor.actorId)||actor.actorId<1||ids.has(actor.actorId))throw new TypeError('Invalid Quick Settings actor ownership or hierarchy');
            ids.add(actor.actorId);for(const child of actor.children)visit(child,depth+1);
        };
        for(const region of ['before','gridItems','after']){
            if(!Array.isArray(record[region])||record[region].length>32)throw new TypeError('Invalid Quick Settings surface region');
            for(const actor of record[region])visit(actor,0);
        }
        if(record.gridItems.length&&(!Number.isInteger(record.columns)||record.columns<1||record.columns>8))throw new TypeError('Invalid Quick Settings grid columns');
    }
    return copies;
}

function preferredWidth(records){
    let width=null;
    for(const record of records){
        const declaration=/(?:^|;)\s*width\s*:\s*(\d+(?:\.\d+)?)px\s*(?:;|$)/i.exec(record.boxStyle||'');
        if(declaration)width=Math.max(1,Math.min(2048,Math.round(Number(declaration[1]))));
    }
    return width;
}

/** Owns extension canvases and retained native slots. It never supplies native
 * network/audio/power state: those controls remain in the Settings actor. */
export class RiceGnomeQuickSettings {
    constructor({version,raster,sendMount,input,layout=()=>{},alive=()=>true,accepts=()=>true,otherFrameBytes=()=>0,otherFrameCount=()=>0,setWidth=()=>{},onDamage=()=>{},onError=()=>{},identity}={}){
        this.version=version;this.raster=raster;this.sendMount=sendMount;this.input=input;this.layout=layout;
        this.alive=alive;this.accepts=accepts;this.otherFrameBytes=otherFrameBytes;this.otherFrameCount=otherFrameCount;this.setWidth=setWidth;this.onDamage=onDamage;this.onError=onError;
        this.identity=identity||(()=>GNOME_QUICK_SETTINGS_FRAME_PREFIX+[...crypto.getRandomValues(new Uint8Array(23))].map(byte=>byte.toString(16).padStart(2,'0')).join(''));
        this.records=[];this.sessions=new Map();this.epoch=0;this.frameRevision=0;this.closed=false;this.inputTasks=new Set();
    }
    async publish(records){
        if(this.closed&&Array.isArray(records)&&!records.length)return;
        if(this.closed)throw new Error('Quick Settings surface owner is closed');
        const next=checkedRecords(records);
        const publication=++this.epoch;
        if(next.length&&await this.version?.()!==1)throw new Error('Native Quick Settings mounting is unavailable in this app module');
        if(this.closed)throw new Error('Quick Settings surface owner closed during publication');
        if(publication!==this.epoch)return;
        this.records=next;
        for(const session of [...this.sessions.values()]){
            this.setWidth(session.window,preferredWidth(next));
            session.width=this.#width(session.window);
            if(!next.length)await this.#clear(session);else await this.#queue(session);
        }
    }
    async #clear(session){
        session.revision++;
        this.#cancelCapture(session);
        for(const target of session.keys.values())this.#send(session,target,{kind:'cancel'});
        session.keys.clear();session.focus=null;
        if(session.hover){this.#send(session,session.hover,{kind:'leave'});session.hover=null;}
        const hadEntries=session.entries.length>0;
        session.entries=[];session.identities.clear();
        if(hadEntries&&this.#live(session))await this.sendMount(session.window,{schema:MOUNT_SCHEMA,before:[],grid:[],after:[]});
        this.onDamage();
    }
    async attach(window){
        if(this.closed||!quickWindow(window)||!this.accepts(window))return false;
        const key=String(window.app);
        if(this.sessions.has(key))return true;
        const session={window,width:this.#width(window),entries:[],identities:new Map(),revision:0,work:Promise.resolve(),capture:null,hover:null,focus:null,keys:new Map()};
        this.sessions.set(key,session);
        try{
            if(this.records.length){
                if(await this.version?.()!==1)throw new Error('Native Quick Settings mounting is unavailable in this app module');
                this.setWidth(window,preferredWidth(this.records));session.width=this.#width(window);await this.#queue(session);
            }
        }catch(error){this.detach(window);throw error;}
        return true;
    }
    #width(window){return Math.max(1,Math.min(2048,Math.round(nativeClientRect(window.frame).width)));}
    #live(session){return !this.closed&&this.sessions.get(String(session.window.app))===session&&this.alive(session.window);}
    #queue(session){
        const revision=++session.revision;
        session.work=session.work.catch(()=>{}).then(()=>this.#rebuild(session,revision));return session.work;
    }
    async #rebuild(session,revision){
        if(!this.#live(session)||revision!==session.revision)return false;
        const epoch=this.epoch,width=session.width,entries=[],mount={schema:MOUNT_SCHEMA,before:[],grid:[],after:[]};
        let bytes=0,totalHeight=0;
        const otherEntries=[...this.sessions.values()].filter(other=>other!==session).flatMap(other=>other.entries);
        const otherBytes=otherEntries.reduce((sum,entry)=>sum+entry.scene.width*entry.scene.height*4,0);
        for(const record of this.records)for(const [region,field] of [['before','before'],['grid','gridItems'],['after','after']]){
            if(!record[field].some(actor=>actor.visible!==false))continue;
            const key=record.uuid+':'+region,tree={kind:'box',vertical:true,styleClass:record.boxClasses||'',children:record[field]};
            if(region==='grid'){tree.layout='grid';tree.columns=record.columns;tree.styleClass=record.gridClasses||'';}
            const signature=JSON.stringify([tree,record.stylesheet,width]);
            const previous=session.entries.find(entry=>entry.key===key&&entry.signature===signature);
            let scene=previous?.scene||await this.raster({tree,stylesheet:record.stylesheet},{allocatedWidth:width,padding:0,backgroundColor:null,retainSurface:true,maximumWidth:2048,maximumHeight:2048});
            if(!this.#live(session)||epoch!==this.epoch||revision!==session.revision)return false;
            if(!Number.isInteger(scene.width)||!Number.isInteger(scene.height)||scene.width<1||scene.width>2048||scene.height<1||scene.height>2048||!scene.surface||!(scene.pixels instanceof Uint32Array)||scene.pixels.length!==scene.width*scene.height)throw new Error('Quick Settings raster returned an invalid owned frame');
            // Isolated desktop workers share immutable pixels. Repeated scene
            // notifications copy metadata rather than the whole image. Other
            // environments retain the ordinary structured-clone data path.
            if(globalThis.crossOriginIsolated===true&&typeof SharedArrayBuffer==='function'&&!(scene.pixels.buffer instanceof SharedArrayBuffer)){
                const pixels=new Uint32Array(new SharedArrayBuffer(scene.pixels.byteLength));pixels.set(scene.pixels);scene={...scene,pixels};
            }
            bytes+=scene.width*scene.height*4;totalHeight+=scene.height;
            const externalBytes=this.otherFrameBytes();
            const externalCount=this.otherFrameCount();
            if(!Number.isSafeInteger(externalBytes)||externalBytes<0||bytes+otherBytes+externalBytes>MAX_FRAME_BYTES||entries.length>=48||!Number.isSafeInteger(externalCount)||externalCount<0||entries.length+otherEntries.length+externalCount>=256||totalHeight>8192)throw new RangeError('Quick Settings retained frame budget exceeded');
            let identity=session.identities.get(key);
            if(!identity){identity=this.identity();if(!new RegExp('^'+GNOME_QUICK_SETTINGS_FRAME_PREFIX+'[a-f0-9]{46}$').test(identity))throw new TypeError('Invalid Quick Settings frame identity');}
            if(entries.some(entry=>entry.identity===identity)||otherEntries.some(entry=>entry.identity===identity))throw new Error('Quick Settings frame identity collision');
            entries.push({key,uuid:record.uuid,region,identity,signature,scene,revision:previous?.scene===scene?previous.revision:++this.frameRevision,rect:previous?.rect||null});
            mount[region].push({identity,height:scene.height});
        }
        const previous=session.entries;session.entries=entries;
        if(!previous.length&&!entries.length)return true;
        // The native UI patch may trigger a paint while dispatch is awaited.
        // Publish its backing frames first; roll them back if dispatch fails.
        try{await this.sendMount(session.window,mount);}
        catch(error){if(this.#live(session)&&revision===session.revision)session.entries=previous;throw error;}
        if(!this.#live(session)||epoch!==this.epoch||revision!==session.revision)return false;
        session.identities=new Map(entries.map(entry=>[entry.key,entry.identity]));
        if(session.capture&&!entries.some(entry=>entry.identity===session.capture.identity))this.#cancelCapture(session);
        if(session.hover&&!entries.some(entry=>entry.identity===session.hover.identity)){this.#send(session,session.hover,{kind:'leave'});session.hover=null;}
        this.onDamage();return true;
    }
    frames(){
        return [...this.sessions.values()].filter(session=>this.#live(session)).flatMap(session=>session.entries.map(entry=>({
            app:String(session.window.app),identity:entry.identity,width:entry.scene.width,height:entry.scene.height,pixels:entry.scene.pixels,revision:entry.revision,
        })));
    }
    observe(window,snapshot){
        const session=this.sessions.get(String(window.app));if(!session||!this.#live(session))return;
        const client=nativeClientRect(window.frame),viewport={x:0,y:0,width:snapshot?.width||client.width,height:snapshot?.height||client.height};
        const portals=new Map(),clips=[viewport];
        for(const command of snapshot?.commands||[]){
            if(command.type==='push-clip')clips.push(intersect(clips.at(-1),command.rect));
            else if(command.type==='pop-clip'){if(clips.length>1)clips.pop();}
            else if(command.type==='host-portal'&&(command.ownerApp===undefined||String(command.ownerApp)===String(window.app)))
                portals.set(command.identity,{rect:command.rect,visible:intersect(clips.at(-1),command.rect)});
        }
        let resize=false;
        for(const entry of session.entries){
            const portal=portals.get(entry.identity),rect=portal?.rect||null;
            entry.visibleRect=portal?.visible||null;
            if(rect&&Number.isFinite(rect.width)&&rect.width>0){
                entry.rect=rect;
                const width=Math.max(1,Math.min(2048,Math.round(rect.width)));
                if(width!==session.width){session.width=width;resize=true;}
                const geometry=JSON.stringify([rect,window.frame.x,window.frame.y,window.frame.titleHeight]);
                if(geometry!==entry.geometry){
                    entry.geometry=geometry;
                    const job=Promise.resolve().then(()=>{
                        if(this.#live(session)&&session.entries.includes(entry))return this.layout(entry.uuid,this.#allocations(session,entry));
                    });
                    this.inputTasks.add(job);void job.catch(this.onError).finally(()=>this.inputTasks.delete(job));
                }
            }else entry.rect=null;
        }
        if(resize)void this.#queue(session).catch(this.onError);
    }
    #allocations(session,entry){
        const frame=session.window.frame,client=nativeClientRect(frame),rect=entry.rect;
        return (entry.scene.allocations||[]).map(allocation=>({...allocation,
            x:allocation.x+frame.x+client.x+(rect?.x||0),y:allocation.y+frame.y+frame.titleHeight+(rect?.y||0)}));
    }
    #send(session,target,event){
        const entry=session.entries.find(entry=>entry.identity===target.identity);
        const origin=entry?.rect?{allocations:this.#allocations(session,entry)}:{};
        const job=Promise.resolve().then(()=>this.input(target.uuid,{...origin,actorId:target.actorId,...event}));
        this.inputTasks.add(job);void job.catch(this.onError).finally(()=>this.inputTasks.delete(job));
    }
    #cancelCapture(session){
        if(session.capture)this.#send(session,session.capture,{kind:'cancel'});
        session.capture=null;
    }
    pointer(window,event){
        const session=this.sessions.get(String(window.app));
        if(!session||!this.#live(session)||!quickWindow(window))return false;
        if(event.input==='pointer-cancel'){const held=Boolean(session.capture);this.#cancelCapture(session);return held;}
        if(event.input==='pointer-leave'){
            if(!session.hover)return false;
            this.#send(session,session.hover,{kind:'leave'});session.hover=null;return true;
        }
        const entry=[...session.entries].reverse().find(entry=>inRect(entry.visibleRect,event.x,event.y));
        const hit=entry&&[...(entry.scene.actorHits||[])].reverse().find(hit=>inRect(hit,event.x-entry.rect.x,event.y-entry.rect.y));
        if(!hit){
            if(event.input==='pointer-button'&&event.pressed)session.focus=null;
            if(session.hover){this.#send(session,session.hover,{kind:'leave'});session.hover=null;}
            if(session.capture&&event.input==='pointer-button'&&!event.pressed){this.#cancelCapture(session);return true;}
            return Boolean(session.capture);
        }
        const target={uuid:entry.uuid,identity:entry.identity,actorId:hit.actorId};
        const client=nativeClientRect(window.frame);
        const coordinates={stageX:window.frame.x+client.x+event.x,stageY:window.frame.y+window.frame.titleHeight+event.y,
            button:Number(event.button)||1,state:Number(event.state)||0};
        if(event.input==='pointer-motion'){
            if(session.hover&&(session.hover.identity!==target.identity||session.hover.actorId!==target.actorId))this.#send(session,session.hover,{kind:'leave',...coordinates});
            session.hover=target;this.#send(session,target,{kind:'move',...coordinates});return true;
        }
        if(event.input!=='pointer-button')return false;
        if([4,5,6,7].includes(Number(event.button))){
            if(event.pressed)this.#send(session,target,{kind:'scroll',direction:{4:0,5:1,6:2,7:3}[event.button],...coordinates});return true;
        }
        if(event.pressed){session.focus=target;session.capture={...target,button:Number(event.button)};this.#send(session,target,{kind:'down',...coordinates});}
        else{
            const captured=session.capture;session.capture=null;
            if(captured&&(captured.identity!==target.identity||captured.actorId!==target.actorId||captured.button!==Number(event.button)))this.#send(session,captured,{kind:'cancel'});
            else if(captured)this.#send(session,target,{kind:'up',...coordinates});
        }
        return true;
    }
    key(window,event){
        const session=this.sessions.get(String(window.app)),keycode=Number(event.keycode);
        if(!session||!this.#live(session))return false;
        if([9,23,27].includes(keycode)){
            if(event.pressed){for(const target of session.keys.values())this.#send(session,target,{kind:'cancel'});session.keys.clear();session.focus=null;}
            return false;
        }
        const target=session.keys.get(keycode)||session.focus;
        if(!target||!session.entries.some(entry=>entry.identity===target.identity))return false;
        if(event.pressed)session.keys.set(keycode,target);else session.keys.delete(keycode);
        this.#send(session,target,{kind:event.cancelled?'cancel':event.pressed?'key-down':'key-up',keycode,state:Number(event.state)||0,text:String(event.text||'').slice(0,64)});
        return true;
    }
    get preferredWidth(){return preferredWidth(this.records);}
    detach(window){
        const session=this.sessions.get(String(window.app));if(!session)return;
        this.#cancelCapture(session);
        for(const target of session.keys.values())this.#send(session,target,{kind:'cancel'});
        session.keys.clear();session.focus=null;
        if(session.hover)this.#send(session,session.hover,{kind:'leave'});
        session.revision++;this.sessions.delete(String(window.app));this.setWidth(window,null);this.onDamage();
    }
    async flush(){
        for(;;){
            const jobs=[...this.sessions.values()].map(session=>session.work).concat([...this.inputTasks]);
            await Promise.all(jobs);if(!this.inputTasks.size&&[...this.sessions.values()].every(session=>jobs.includes(session.work)))return;
        }
    }
    close(){if(this.closed)return;for(const session of [...this.sessions.values()])this.detach(session.window);this.records=[];this.closed=true;this.epoch++;}
}
