import {shapedTextPresentation} from '../sdk/versions/1.0.0/runtime/sdk/js/shaped-text-presentation.mjs';
import {ShapedTextLayout,MAX_SHAPED_LAYOUT_BYTES,MAX_SHAPED_TEXT_BYTES} from '../sdk/versions/1.0.0/runtime/sdk/js/shaped-text-layout.mjs';
import {documentEditorMetadata,isDocumentEditor} from './rice-document-editor-layout.mjs';

const encode=new TextEncoder(),KINDS=['lines','clusters','stops'];
const exact=(v,keys)=>v&&typeof v==='object'&&!Array.isArray(v)&&Object.keys(v).length===keys.length&&keys.every(k=>Object.hasOwn(v,k));
const id=value=>{if(typeof value!=='string'||! /^[1-9][0-9]{0,19}$/u.test(value)||BigInt(value)>0xffffffffffffffffn)throw RangeError('Invalid shaped editor identity');return value;};
const token=value=>{if(typeof value!=='string'||! /^[a-f0-9]{32}$/u.test(value))throw RangeError('Invalid shaped editor resource');return value;};
const jsonBytes=value=>encode.encode(JSON.stringify(value)).length;
const MAX_REQUEST_BYTES=64*1024,MAX_TOTAL_BYTES=32*1024*1024,MAX_OWNER_BYTES=8*1024*1024;
const headerKeys=['version','textBytes','lineCount','clusterCount','stopCount','bytes'];

/** Intrinsic native UI data only. Trusted callers supply the authenticated
 * native app/window owner; request fields never select another owner. Uploads
 * are paged, immutable after commit, bounded and released with the native app.
 * Glyphs continue through SceneSurface and its ordinary native ingress. */
export class NativeShapedEditorLayouts {
    constructor({clock=()=>Date.now(),nonce=()=>crypto.randomUUID().replaceAll('-','')}={}) {
        this.clock=clock;this.nonce=nonce;this.records=new Map();
    }
    owner(value){return id(String(value?.app))+':'+id(String(value?.window));}
    expire(){for(const[key,row]of this.records)if(!row.layout&&this.clock()-row.created>30000)this.records.delete(key);}
    quota(principal,bytes,additional=0){
        const rows=[...this.records.values()],own=rows.filter(r=>r.principal===principal);
        if(rows.length+additional>32||own.length+additional>8||rows.reduce((n,r)=>n+r.bytes,bytes)>MAX_TOTAL_BYTES||own.reduce((n,r)=>n+r.bytes,bytes)>MAX_OWNER_BYTES)throw RangeError('Shaped editor resource quota exceeded');
    }
    owned(owner,widget,key){const row=this.records.get(token(key));if(!row||row.owner!==owner||row.widget!==id(widget))throw Error('Shaped editor resource is not owned by this control');return row;}
    request(ownerValue,request){
        const owner=this.owner(ownerValue);this.expire();
        if(!request||jsonBytes(request)>MAX_REQUEST_BYTES)throw RangeError('Shaped editor request exceeds 64 KiB');
        const {action}=request;
        if(action==='begin') {
            if(!(exact(request,['action','widget','text','header'])||exact(request,['action','widget','text','header','presentation']))||typeof request.text!=='string'||encode.encode(request.text).length>MAX_SHAPED_TEXT_BYTES||!exact(request.header,headerKeys))throw RangeError('Invalid shaped editor upload');
            const h=request.header;
            if(h.version!==1||h.textBytes!==encode.encode(request.text).length||!Number.isInteger(h.bytes)||h.bytes<0||h.bytes>MAX_SHAPED_LAYOUT_BYTES||
                !Number.isInteger(h.lineCount)||h.lineCount<1||h.lineCount>16385||!Number.isInteger(h.clusterCount)||h.clusterCount<0||h.clusterCount>32768||!Number.isInteger(h.stopCount)||h.stopCount<1||h.stopCount>32770)throw RangeError('Invalid shaped editor upload counts');
            const widget=id(request.widget),presentation=shapedTextPresentation(request.presentation??null),bytes=jsonBytes(request);const principal=id(String(ownerValue.principal??ownerValue.app));this.quota(principal,bytes,1);
            const key=token(this.nonce());if(this.records.has(key))throw Error('Shaped editor resource identity collision');
            this.records.set(key,{owner,principal,widget,presentation,text:request.text,header:{...h},lines:[],clusters:[],stops:[],bytes,created:this.clock(),layout:null,metadataRevision:0,sequence:0,selection:null});
            return {token:key};
        }
        if(!['append','commit','release','state'].includes(action))throw RangeError('Unknown shaped editor action');
        const fields=action==='append'?['action','widget','token','kind','offset','rows']:['action','widget','token'];
        if(!exact(request,fields))throw RangeError('Invalid shaped editor request fields');
        const row=this.owned(owner,request.widget,request.token);
        if(action==='release'){this.records.delete(request.token);return {released:true};}
        if(action==='state')return {ready:Boolean(row.layout),sequence:row.sequence,selection:row.selection?{...row.selection}:null};
        if(row.layout)throw Error('Committed shaped editor resources are immutable');
        if(action==='append') {
            const {kind,offset,rows}=request;
            if(!KINDS.includes(kind)||!Array.isArray(rows)||rows.length<1||rows.length>128||offset!==row[kind].length||offset+rows.length>row.header[{lines:'lineCount',clusters:'clusterCount',stops:'stopCount'}[kind]])throw RangeError('Invalid shaped editor page');
            const bytes=jsonBytes(rows);if(row.bytes+bytes>MAX_SHAPED_LAYOUT_BYTES)throw RangeError('Shaped editor upload exceeds 4 MiB');this.quota(row.principal,bytes);
            const copied=structuredClone(rows);row[kind].push(...copied);row.bytes+=bytes;return {next:row[kind].length};
        }
        // Validation precedes publication. A rejected candidate cannot replace
        // a prior committed map or mutate its source/geometry/selection.
        const layout=new ShapedTextLayout(row.text,{...row.header,lines:row.lines,clusters:row.clusters,stops:row.stops});
        row.layout=layout;delete row.lines;delete row.clusters;delete row.stops;
        return {ready:true};
    }
    attach(snapshot){
        const owner=this.owner(snapshot);let bytes=0,changed=false;const underlays=new Map();
        const commands=snapshot.commands.map(command=>{
            if(!isDocumentEditor(command))return command;
            const metadata=documentEditorMetadata(command),reference=metadata.shaped;if(!reference)return command;
            changed=true;let row;
            try{row=this.owned(owner,String(metadata.id),reference.token);}catch{return Object.freeze({...command,shapedEditorPending:true});}
            if(!row.layout||row.text!==metadata.presentation.document)return Object.freeze({...command,shapedEditorPending:true});
            const presentation=row.presentation;
            const glyphs=presentation?.surface?snapshot.commands.find(c=>c.type==='web-scene'&&c.vectorScene&&String(c.surface)===presentation.surface&&String(c.epoch)===presentation.epoch&&['x','y','width','height'].every(k=>c.rect[k]===command.rect[k])):null;
            if(presentation?.surface&&!glyphs)return Object.freeze({...command,shapedEditorPending:true});
            const layout=row.layout,matches=(caret,anchor)=>{
                try{return layout.caret(caret).scalar===metadata.presentation.caret&&layout.caret(anchor).scalar===metadata.presentation.anchor;}catch{return false;}
            };
            if(reference.revision>row.metadataRevision&&matches(reference.caret,reference.anchor)){
                row.metadataRevision=reference.revision;row.selection={caret:reference.caret,anchor:reference.anchor};
            }
            let selection=row.selection;
            if(!selection||!matches(selection.caret,selection.anchor)) {
                const caret=layout.stopAtScalar(metadata.presentation.caret),anchor=layout.stopAtScalar(metadata.presentation.anchor);
                if(caret===null||anchor===null)return Object.freeze({...command,shapedEditorPending:true});
                selection={caret,anchor};
                // Native pointer feedback can be painted before the SDK's
                // scalar-selection acknowledgement updates widget text. Keep
                // the accepted affinity until that acknowledgement arrives.
                if(!row.selection)row.selection=selection;
            }
            bytes+=row.bytes;
            const resolved=Object.freeze({...command,...(glyphs?{shapedSelectionBelow:true}:{}),shapedEditor:Object.freeze({token:reference.token,text:row.text,map:layout.map,bytes:row.bytes,presentation:row.presentation,...selection})});
            if(glyphs)underlays.set(glyphs,{type:'shaped-text-selection',rect:resolved.rect,editor:resolved});
            return resolved;
        });
        return {snapshot:changed?Object.freeze({...snapshot,commands:Object.freeze(commands.flatMap(command=>underlays.has(command)?[underlays.get(command),command]:[command]))}):snapshot,bytes};
    }
    pointer(ownerValue,widget,key,stop,{extend=false}={}) {
        const row=this.owned(this.owner(ownerValue),String(widget),key);if(!row.layout)throw Error('Shaped editor is not ready');row.layout.caret(stop);
        row.selection={caret:stop,anchor:extend&&row.selection?row.selection.anchor:stop};row.sequence++;
    }
    capture(){
        this.expire();
        return structuredClone({schema:'rice.native-shaped-editors/v1',resources:[...this.records].map(([token,row])=>{
            const map=row.layout?.map;
            return {token,owner:row.owner,principal:row.principal,widget:row.widget,presentation:row.presentation,text:row.text,header:row.header,ready:Boolean(map),
                lines:map?.lines||row.lines,clusters:map?.clusters||row.clusters,stops:map?.stops||row.stops,
                metadataRevision:row.metadataRevision,sequence:row.sequence,selection:row.selection};
        })});
    }
    restore(snapshot){
        if(!exact(snapshot,['schema','resources'])||snapshot.schema!=='rice.native-shaped-editors/v1'||!Array.isArray(snapshot.resources)||snapshot.resources.length>32||jsonBytes(snapshot)>MAX_TOTAL_BYTES+65536)throw RangeError('Invalid shaped editor checkpoint');
        const candidate=new NativeShapedEditorLayouts({clock:this.clock,nonce:this.nonce});
        for(const saved of snapshot.resources){
            if(!exact(saved,['token','owner','principal','widget','presentation','text','header','ready','lines','clusters','stops','metadataRevision','sequence','selection'])||typeof saved.owner!=='string'||typeof saved.ready!=='boolean'||!['metadataRevision','sequence'].every(k=>Number.isSafeInteger(saved[k])&&saved[k]>=0))throw RangeError('Invalid shaped editor checkpoint resource');
            const ids=saved.owner.split(':');if(ids.length!==2)throw RangeError('Invalid shaped editor checkpoint owner');
            const owner={app:id(ids[0]),window:id(ids[1]),principal:id(saved.principal)};candidate.nonce=()=>token(saved.token);
            const {token:key}=candidate.request(owner,{action:'begin',widget:saved.widget,text:saved.text,header:saved.header,presentation:saved.presentation});
            for(const kind of KINDS){
                const rows=saved[kind];if(!Array.isArray(rows))throw RangeError('Invalid shaped editor checkpoint rows');
                for(let offset=0;offset<rows.length;offset+=128)candidate.request(owner,{action:'append',widget:saved.widget,token:key,kind,offset,rows:rows.slice(offset,offset+128)});
            }
            if(saved.ready)candidate.request(owner,{action:'commit',widget:saved.widget,token:key});
            const row=candidate.records.get(key);
            if(saved.selection!==null){
                if(!saved.ready||!exact(saved.selection,['caret','anchor']))throw RangeError('Invalid shaped editor checkpoint selection');
                row.layout.caret(saved.selection.caret);row.layout.caret(saved.selection.anchor);row.selection={...saved.selection};
            }
            row.metadataRevision=saved.metadataRevision;row.sequence=saved.sequence;
        }
        this.records=candidate.records;
    }
    removeApp(app){const prefix=id(String(app))+':';for(const[key,row]of this.records)if(row.owner.startsWith(prefix))this.records.delete(key);}
}
