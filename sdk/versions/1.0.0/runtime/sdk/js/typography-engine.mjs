import {resolveDocumentFont} from './document-fonts.mjs';
import {checkedShapedTextLayout} from './shaped-text-layout.mjs';
/** Real Parley/Skrifa worker; package font resources stay outside Wasm code.
 * Returned paths use RiceWM's native vector protocol, never scalar glyph IDs.
 */
const CAPACITY = 65536, MAX_FRAME = 1024 * 1024 - 56;
const decoder = new TextDecoder('utf-8', { fatal: true });
const encoder = new TextEncoder();
const EXPORTS = ['typography_abi_version','typography_buffer_ptr','typography_buffer_capacity','typography_font_begin','typography_font_write','typography_request','typography_output_read'];
const validPath = path => typeof path === 'string' && /^(?!\/)(?!.*(?:^|\/)\.{1,2}(?:\/|$))[A-Za-z0-9._/-]{1,255}$/.test(path);
function fail(message) { return new Error(`Typography: ${message}`); }

export async function openTypographyEngine({ services, path = 'typography.wasm', fontManifest = 'fonts-manifest.json', fonts = null, signal } = {}) {
    if (!validPath(path) || fonts===null&&!validPath(fontManifest) || !services?.compute || !services?.assets) throw fail('approved compute and package assets are required');
    if(fonts!==null)fonts=structuredClone(fonts);
    let handle, pointer, closed = false, pending = false;
    const alive = () => { if (closed || signal?.aborted) throw fail('worker closed'); };
    const invoke = (name, args = [], extra = {}) => services.compute.invoke({abi:1,handle,name,args,timeoutMs:4000,...extra});
    async function close() {
        closed = true; signal?.removeEventListener('abort', abort);
        const owned = handle; handle = undefined;
        if (owned) await services.compute.cancel({handle:owned});
    }
    function abort() { void close().catch(()=>{}); }
    signal?.addEventListener('abort', abort, {once:true});
    async function request(operation, arguments_ = {}) {
        alive();
        const bytes = encoder.encode(JSON.stringify({operation,arguments:arguments_}));
        if (bytes.length > CAPACITY) throw fail('request exceeds64KiB');
        const reply = await invoke('typography_request',[bytes.length],{write:{offset:pointer,data:bytes},read:{offset:pointer,lengthFromResult:true,maxLength:CAPACITY}});
        alive();
        if (!(reply.data instanceof Uint8Array) || reply.value !== reply.data.length || reply.value < 1 || reply.value > CAPACITY) throw fail('invalid response length');
        const value = JSON.parse(decoder.decode(reply.data));
        if (value?.ok !== true) { const error=fail(value?.error?.message || 'request failed');error.code=value?.error?.code;error.styleIndex=value?.error?.styleIndex;throw error; }
        return value.result;
    }
    try {
        alive();
        const opened = await services.compute.open({abi:1,path,timeoutMs:4000});
        handle=opened?.handle;alive();
        if (typeof handle !== 'string' || opened.abi !== 1 || !Array.isArray(opened.exports) || EXPORTS.some(name=>!opened.exports.includes(name))) throw fail('incompatible ABI');
        if ((await invoke('typography_abi_version')).value !== 1 || (await invoke('typography_buffer_capacity')).value !== CAPACITY) throw fail('incompatible staging buffer');
        pointer=(await invoke('typography_buffer_ptr')).value;
        if (!Number.isSafeInteger(pointer) || pointer <= 0 || pointer > 0xffffffff-CAPACITY) throw fail('invalid staging address');
        let manifest;
        if(fonts===null){
        const manifestInfo=await services.assets.stat({path:fontManifest});
        if (!Number.isSafeInteger(manifestInfo?.size) || manifestInfo.size < 1 || manifestInfo.size > CAPACITY) throw fail('invalid font manifest size');
        const manifestBytes=await services.assets.read({path:fontManifest,offset:0,length:manifestInfo.size});
        if (!(manifestBytes instanceof Uint8Array) || manifestBytes.length !== manifestInfo.size) throw fail('incomplete font manifest');
        manifest=JSON.parse(decoder.decode(manifestBytes));
        }else manifest={version:1,fonts};
        if(manifest?.version!==1 || !Array.isArray(manifest.fonts) || !manifest.fonts.length || manifest.fonts.length>16)throw fail('invalid font manifest');
        let total=0;const seen=new Set();
        for(const font of manifest.fonts){
            alive();
            const shared=font?.resource!==undefined;
            if(shared?font.path!==undefined||typeof font.resource!=='string'||!font.resource||font.resource.length>255:!validPath(font?.path))throw fail('invalid font resource');
            const identity=(shared?'sdk:':'package:')+(shared?font.resource:font.path);
            if(seen.has(identity))throw fail('duplicate font resource');seen.add(identity);
            const reader=shared?services.resources:services.assets;
            if(typeof reader?.stat!=='function'||typeof reader?.read!=='function')throw fail('selected SDK font resources require resources.stat/read');
            const address=shared?{key:font.resource}:{path:font.path};
            const meta=await reader.stat(address);
            if(!Number.isSafeInteger(meta?.size)||meta.size<1||meta.size>20*1024*1024||(total+=meta.size)>32*1024*1024)throw fail('font byte limit');
            if(font.bytes!==undefined&&font.bytes!==meta.size||font.sha256!==undefined&&font.sha256!==meta.sha256)throw fail('font metadata does not match its resource');
            if((await invoke('typography_font_begin',[meta.size])).value!==0)throw fail('font upload rejected');
            for(let offset=0;offset<meta.size;offset+=CAPACITY){
                alive();const length=Math.min(CAPACITY,meta.size-offset);
                const data=await reader.read({...address,offset,length});
                if(!(data instanceof Uint8Array)||data.length!==length)throw fail('incomplete font resource');
                if((await invoke('typography_font_write',[offset,length],{write:{offset:pointer,data}})).value!==0)throw fail('font chunk rejected');
            }
            await request('register-font');
        }
        return Object.freeze({
            version:1, fonts:Object.freeze(manifest.fonts.map(font=>Object.freeze({...font}))),
            resolveDocumentFamily(requested) {
                const resolved=resolveDocumentFont(requested).family;
                if(manifest.fonts.some(font=>font.family===resolved))return resolved;
                const actual=manifest.fonts.find(font=>typeof font.family==='string'&&font.family.toLowerCase()===String(requested).trim().toLowerCase())?.family;
                if(actual)return actual;
                const fallback=manifest.fonts.find(font=>font.family==='Liberation Sans')?.family||manifest.fonts[0]?.family;
                if(!fallback)throw fail('No document font is bundled');
                return fallback;
            },
            async shape({text,families='Liberation Sans, sans-serif',size=24,width=null,bold=false,italic=false,alignment='start',color=0xff000000,pathBase=1,underline=false,spans=[],lineHeight=null,editing=false}={}){
                if(typeof editing!=='boolean')throw fail('editing must be a boolean');
                alive();if(pending)throw fail('operation already pending');pending=true;
                try {
                    const result=await request('shape',{text,families,size,width,bold,italic,alignment,color,pathBase,underline,spans,...(lineHeight===null?{}:{lineHeight}),...(editing?{editing:true}:{})});
                    if(!Number.isInteger(result?.snapshot)||result.snapshot<1||!Number.isInteger(result.frameBytes)||result.frameBytes<1||result.frameBytes>MAX_FRAME||!Number.isInteger(result.glyphCount)||result.glyphCount<0||result.glyphCount>4096)throw fail('invalid shape metadata');
                    const displayList=new Uint8Array(result.frameBytes);
                    for(let offset=0;offset<displayList.length;offset+=CAPACITY){
                        const length=Math.min(CAPACITY,displayList.length-offset);
                        const reply=await invoke('typography_output_read',[result.snapshot,offset,length],{read:{offset:pointer,lengthFromResult:true,maxLength:length}});
                        alive();if(reply.value!==length||!(reply.data instanceof Uint8Array)||reply.data.length!==length)throw fail('incomplete vector snapshot');
                        displayList.set(reply.data,offset);
                    }
                    const glyphs=[];
                    for(let start=0;start<result.glyphCount;start+=128){
                        const page=await request('glyphs',{snapshot:result.snapshot,start,limit:128});
                        if(page?.snapshot!==result.snapshot||!Array.isArray(page.glyphs)||page.glyphs.length!==Math.min(128,result.glyphCount-start))throw fail('invalid glyph metadata page');
                        glyphs.push(...page.glyphs);
                    }
                    let editingMap;
                    if(editing) {
                        const metadata=result.editing;
                        if(!metadata||metadata.version!==1||metadata.textBytes!==encoder.encode(text).length||
                            !Number.isInteger(metadata.bytes)||metadata.bytes<0||metadata.bytes>4*1024*1024||
                            !Number.isInteger(metadata.lineCount)||metadata.lineCount<0||metadata.lineCount>16385||
                            !Number.isInteger(metadata.clusterCount)||metadata.clusterCount<0||metadata.clusterCount>32768||
                            !Number.isInteger(metadata.stopCount)||metadata.stopCount<0||metadata.stopCount>32770)throw fail('invalid native editing metadata');
                        editingMap={...metadata};
                        for(const[kind,count]of [['lines',metadata.lineCount],['clusters',metadata.clusterCount],['stops',metadata.stopCount]]) {
                            const rows=[];
                            for(let start=0;start<count;start+=128) {
                                const page=await request('editing-map',{snapshot:result.snapshot,kind,start,limit:128});
                                if(page?.snapshot!==result.snapshot||page.version!==1||page.kind!==kind||page.textBytes!==metadata.textBytes||
                                    !Array.isArray(page.items)||page.items.length!==Math.min(128,count-start)||page.next!==(start+page.items.length<count?start+page.items.length:null))throw fail('invalid native editing page');
                                rows.push(...page.items);
                            }
                            editingMap[kind]=rows;
                        }
                        editingMap=checkedShapedTextLayout(text,editingMap);
                    }
                    return Object.freeze({...result,...(editing?{editing:editingMap}:{}),displayList,glyphs});
                }finally{pending=false;}
            },
            close,
        });
    }catch(error){await close().catch(()=>{});throw error;}
}
