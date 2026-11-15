import {ZipParts} from './zip-parts.mjs';
import {parseXmlSource} from './xml-source.mjs';

export const OPC_RELATIONSHIPS='http://schemas.openxmlformats.org/package/2006/relationships';
export const OPC_CONTENT_TYPES='http://schemas.openxmlformats.org/package/2006/content-types';
const decoder=new TextDecoder('utf-8',{fatal:true});
const attr=(node,name)=>node.attributes.find(a=>a.localName===name&&!a.namespaceURI)?.value;
const fail=message=>{throw new TypeError('Invalid OPC package: '+message);};
export const opcRelationshipsPart=part=>part?part.slice(0,part.lastIndexOf('/')+1)+'_rels/'+part.slice(part.lastIndexOf('/')+1)+'.rels':'_rels/.rels';
export const opcPartTarget=part=>'/'+part.split('/').map(encodeURIComponent).join('/');

/** Resolve an internal URI without filesystem access. Names in an OPC ZIP are
 * case-insensitive; encoded separators and escaping package roots are refused. */
export function resolveOpcPart(owner,target) {
    if(typeof owner!=='string'||owner.length>4096||owner&&(owner.startsWith('/')||/[\\\x00-\x1f\x7f\ud800-\udfff]/u.test(owner)||owner.split('/').some(part=>!part||part==='.'||part==='..'))||typeof target!=='string'||!target||target.length>4096||/[\\?#\x00-\x20\x7f]/u.test(target)||/^[a-z][a-z0-9+.-]*:/iu.test(target))fail('unsafe internal relationship target');
    const segments=target.startsWith('/')?[]:owner.split('/').slice(0,-1);
    for(const raw of target.replace(/^\//u,'').split('/')) {
        let value;try{value=decodeURIComponent(raw);}catch{fail('invalid target encoding');}
        if(!value||/[\\/\x00-\x1f\x7f]/u.test(value))fail('invalid target segment');
        if(value==='.')continue;
        if(value==='..'){if(!segments.length)fail('relationship escapes package');segments.pop();}
        else segments.push(value);
    }
    if(!segments.length)fail('empty part target');return segments.join('/');
}

/** Immutable relationship/content-type view over bounded ZIP parts. This is a
 * data reader, not document edit or signature authority. A codec must validate
 * the document identity and import its complete candidate before committing. */
export class OpcPackage {
    #names;#types=new Map();#defaults=new Map();#relationships=new Map();
    constructor(input,options={}) {
        if(input instanceof ZipParts&&Object.keys(options).length)throw new TypeError('Choose ZIP bounds when constructing ZipParts');
        this.archive=input instanceof ZipParts?input:new ZipParts(input,{maximumBytes:8*1024*1024,maximumPartBytes:16*1024*1024,maximumExpandedBytes:64*1024*1024,...options});
        this.#names=new Map(this.archive.names.filter(name=>!name.endsWith('/')).map(name=>[name.toLowerCase(),name]));
        const document=this.xml('[Content_Types].xml');
        if(document.root.namespaceURI!==OPC_CONTENT_TYPES||document.root.localName!=='Types')fail('content types root');
        for(const node of document.root.children) {
            if(node.namespaceURI!==OPC_CONTENT_TYPES)continue;
            const type=attr(node,'ContentType');if(typeof type!=='string'||type.length>256||!/^[^\s<>"/]+\/[^\s<>"]+$/u.test(type))fail('content type');
            if(node.localName==='Override') {
                const name=attr(node,'PartName');if(!name?.startsWith('/'))fail('override part name');
                const key=resolveOpcPart('',name).toLowerCase();if(this.#types.has(key))fail('duplicate content-type override');this.#types.set(key,type);
            } else if(node.localName==='Default') {
                const extension=attr(node,'Extension');if(typeof extension!=='string'||!extension||extension.length>128||/[\\/.\s\x00-\x1f\x7f]/u.test(extension))fail('default extension');
                const key=extension.toLowerCase();if(this.#defaults.has(key))fail('duplicate default content type');this.#defaults.set(key,type);
            } else fail('unknown content-type declaration');
        }
        Object.freeze(this);
    }
    canonical(name) {const result=this.#names.get(String(name).toLowerCase());if(!result)fail('missing part '+String(name).slice(0,256));return result;}
    has(name) {return typeof name==='string'&&this.#names.has(name.toLowerCase());}
    read(name) {return this.archive.read(this.canonical(name));}
    xml(name) {return parseXmlSource(decoder.decode(this.read(name)));}
    contentType(name) {
        const part=this.canonical(name),extension=part.slice(part.lastIndexOf('/')+1).split('.').slice(1).at(-1)?.toLowerCase();
        const result=this.#types.get(part.toLowerCase())||this.#defaults.get(extension);if(!result)fail('missing content type for '+part);return result;
    }
    relationships(owner='') {
        owner=owner?this.canonical(owner):'';
        if(this.#relationships.has(owner))return this.#relationships.get(owner);
        const path=opcRelationshipsPart(owner);if(!this.has(path)){const empty=Object.freeze([]);this.#relationships.set(owner,empty);return empty;}
        const document=this.xml(path),root=document.root;if(root.namespaceURI!==OPC_RELATIONSHIPS||root.localName!=='Relationships')fail('relationships root');
        const ids=new Set(),result=[];
        for(const node of root.children)if(node.namespaceURI===OPC_RELATIONSHIPS&&node.localName==='Relationship') {
            const id=attr(node,'Id'),type=attr(node,'Type'),target=attr(node,'Target'),mode=attr(node,'TargetMode')??'Internal';
            if(typeof id!=='string'||!id||id.length>4096||ids.has(id)||typeof type!=='string'||type.length>4096||!/^[a-z][a-z0-9+.-]*:/iu.test(type)||/[\s\x00-\x1f\x7f]/u.test(type)||typeof target!=='string'||!target||target.length>4096||/[\x00-\x1f\x7f]/u.test(target)||!['Internal','External'].includes(mode))fail('relationship identity, type or target');
            ids.add(id);result.push(Object.freeze({id,type,target,mode,part:mode==='External'?null:this.canonical(resolveOpcPart(owner,target)),node}));
        }
        const frozen=Object.freeze(result);this.#relationships.set(owner,frozen);return frozen;
    }
    get hasSignatures() {return this.archive.names.some(name=>name.toLowerCase().startsWith('_xmlsignatures/'))||this.relationships().some(rel=>rel.type.includes('/digital-signature/'));}
    closure(roots,{include=()=>true,maximumParts=16384}={}) {
        if(!Array.isArray(roots)||!roots.length||roots.length>4096||typeof include!=='function'||!Number.isSafeInteger(maximumParts)||maximumParts<1||maximumParts>16384)fail('bounded graph roots');
        const pending=[...new Set(roots.map(name=>this.canonical(name)))],scheduled=new Set(pending),parts=new Set();
        for(let cursor=0;cursor<pending.length;cursor++) {
            const owner=pending[cursor];parts.add(owner);this.contentType(owner);
            const path=opcRelationshipsPart(owner);if(this.has(path))parts.add(this.canonical(path));
            if(parts.size>maximumParts)fail('relationship graph exceeds part limit');
            for(const rel of this.relationships(owner))if(rel.part&&include(rel,owner)&&!scheduled.has(rel.part)){scheduled.add(rel.part);if(scheduled.size>maximumParts)fail('relationship graph exceeds part limit');pending.push(rel.part);}
        }
        return Object.freeze([...parts]);
    }
    static controlFactory=Object.freeze({topics:['documents','workflow'],constructor:'new OpcPackage(bytesOrZipParts,zipBounds?)',methods:'archive, canonical(name), has(name), read(name), xml(name), contentType(name), relationships(owner?), hasSignatures, closure(roots,{include?,maximumParts?}). Bounded immutable Office package graph, cyclic shared resources and external relationship descriptors. Does not fetch external resources or authorize edits/signature removal. Validate complete codec candidates and commit through the document owner.'});
}
