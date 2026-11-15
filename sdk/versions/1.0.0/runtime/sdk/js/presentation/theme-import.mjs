import {ZipParts} from '../zip-parts.mjs';
import {OpcPackage,opcRelationshipsPart,opcPartTarget,OPC_RELATIONSHIPS,OPC_CONTENT_TYPES} from '../opc-package.mjs';
import {parseXmlSource,patchXmlSource,escapeXmlAttribute} from '../xml-source.mjs';
import {readPptxThemeTarget,installTheme,replaceMasterIdentity} from './theme-package.mjs';
import {THEME_BYTES} from './theme-contract.mjs';
import {IMAGE_BYTES} from './image-contract.mjs';

const A='http://schemas.openxmlformats.org/drawingml/2006/main',SA='http://purl.oclc.org/ooxml/drawingml/main';
const P='http://schemas.openxmlformats.org/presentationml/2006/main',SP='http://purl.oclc.org/ooxml/presentationml/main';
const R='http://schemas.openxmlformats.org/officeDocument/2006/relationships',SR='http://purl.oclc.org/ooxml/officeDocument/relationships';
const MIME='application/vnd.openxmlformats-officedocument.theme+xml';
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true}),esc=escapeXmlAttribute;
const attr=(n,k)=>n.attributes.find(v=>!v.namespaceURI&&v.localName===k)?.value;
const text=(opc,part)=>decoder.decode(opc.read(part));
const one=(node,ns,name)=>{const nodes=node.children.filter(n=>n.namespaceURI===ns&&n.localName===name);if(nodes.length>1)throw TypeError('Ambiguous theme source '+name);return nodes[0]??null;};
function readSource(bytes){
    const archive=new ZipParts(bytes,{maximumBytes:THEME_BYTES,maximumPartBytes:16*1024*1024,maximumExpandedBytes:64*1024*1024,maximumEntries:16384}),opc=new OpcPackage(archive);
    if(archive.names.some(n=>n.toLowerCase().startsWith('_xmlsignatures/'))||opc.relationships('').some(r=>r.type.includes('/digital-signature/')))throw TypeError('Signed theme sources require a signature-aware workflow');
    const documents=opc.relationships('').filter(rel=>[R+'/officeDocument',SR+'/officeDocument'].includes(rel.type));
    if(documents.length!==1||!documents[0].part)throw TypeError('Missing or external theme source document');
    let path=documents[0].part,root=opc.xml(path).root;
    if([P,SP].includes(root.namespaceURI)&&root.localName==='presentation'){
        const p=root.namespaceURI,r=p===P?R:SR,relations=opc.relationships(path),list=one(root,p,'sldMasterIdLst'),first=list?.children.find(n=>n.namespaceURI===p&&n.localName==='sldMasterId');
        const id=first?.attributes.find(v=>v.namespaceURI===r&&v.localName==='id')?.value;
        const master=id?relations.find(v=>v.id===id&&v.type===r+'/slideMaster'):relations.find(v=>v.type===r+'/slideMaster');
        if(!master?.part)throw TypeError('Theme source has no first master');
        const themes=opc.relationships(master.part).filter(v=>v.type===r+'/theme');if(themes.length!==1||!themes[0].part)throw TypeError('Theme source master has no unique internal theme');
        path=themes[0].part;root=opc.xml(path).root;
    }
    if(![A,SA].includes(root.namespaceURI)||root.localName!=='theme'||opc.contentType(path)!==MIME)throw TypeError('The selected package does not contain a primary theme');
    const a=root.namespaceURI,r=a===A?R:SR,name=(attr(root,'name')||'Custom').trim()||'Custom';
    if(name.length>256||/[\x00-\x1f\x7f]/u.test(name))throw TypeError('Theme source name exceeds the supported profile');
    const media=[];
    for(const relation of opc.relationships(path)){
        if(relation.type!==r+'/image'||!relation.part)throw TypeError('Theme source has unsupported or external resource relationships');
        if(media.length>=512)throw TypeError('Theme source has too many media resources');
        const mime=opc.contentType(relation.part),data=opc.read(relation.part);
        if(!/^image\/[A-Za-z0-9.+-]+$/u.test(mime)||!data.length||data.length>IMAGE_BYTES)throw TypeError('Theme media exceeds the existing image source profile');
        media.push({id:relation.id,part:relation.part,mime,bytes:data});
    }
    const ids=new Set(media.map(m=>m.id));
    for(const node of opc.xml(path).nodes)for(const attribute of node.attributes)if(attribute.namespaceURI===r&&['embed','link','id'].includes(attribute.localName)&&!ids.has(attribute.value))throw TypeError('Theme source references a missing resource');
    return {opc,path,root,a,r,name,media};
}
export function inspectPresentationThemePackage(bytes){
    const source=readSource(bytes);return {name:source.name,sourcePart:source.path,mediaCount:source.media.length,mediaBytes:source.media.reduce((n,m)=>n+m.bytes.length,0)};
}
function namespaces(xml,source,target){
    if(source.a===target.a)return xml;
    const doc=parseXmlSource(xml),mapping=new Map([[source.a,target.a],[source.r,target.r]]),edits=[];
    for(const node of doc.nodes)for(const attribute of node.attributes)if(attribute.namespaceURI==='http://www.w3.org/2000/xmlns/'&&mapping.has(attribute.value))edits.push({start:attribute.valueStart,end:attribute.valueEnd,text:mapping.get(attribute.value)});
    return patchXmlSource(xml,edits);
}
function appendOverride(xml,path,mime){
    const root=parseXmlSource(xml).root;if(root.namespaceURI!==OPC_CONTENT_TYPES||root.localName!=='Types'||root.selfClosing)throw TypeError('Invalid target content types');
    return patchXmlSource(xml,[{start:root.closeStart,end:root.closeStart,text:`<Override xmlns="${OPC_CONTENT_TYPES}" PartName="${opcPartTarget(path)}" ContentType="${esc(mime)}"/>`}]);
}
/** Import the primary THMX theme or the first master theme of PPTX/POTX/PPSX.
 * Resource identities are remapped into fresh target parts. No source code,
 * external resource fetch, conversion executable or second document model. */
export function importPptxTheme(input,themeBytes,slideId){
    const target=readPptxThemeTarget(input,slideId),source=readSource(themeBytes),changes=new Map();
    const xml=namespaces(text(source.opc,source.path),source,{a:target.context.a,r:target.r});
    changes.set(target.master.part,encoder.encode(replaceMasterIdentity(target,source.name)));
    const path=installTheme(target,xml,changes,{forceNew:true,copyExistingRelationships:false}),resources=new Map(),rels=[];
    let types=decoder.decode(changes.get('[Content_Types].xml')),serial=1;
    for(const item of source.media){
        let destination=resources.get(item.part);
        if(!destination){
            const suffix=({ 'image/png':'png','image/jpeg':'jpg','image/svg+xml':'svg','image/gif':'gif','image/bmp':'bmp','image/tiff':'tiff','image/webp':'webp' })[item.mime]??'bin';
            do{destination='ppt/media/rice-theme-image-'+serial+++'.'+suffix;}while(target.opc.has(destination)||changes.has(destination));
            resources.set(item.part,destination);changes.set(destination,item.bytes.slice());types=appendOverride(types,destination,item.mime);
        }
        rels.push(`<Relationship xmlns="${OPC_RELATIONSHIPS}" Id="${esc(item.id)}" Type="${target.r}/image" Target="${opcPartTarget(destination)}"/>`);
    }
    if(rels.length)changes.set(opcRelationshipsPart(path),encoder.encode(`<Relationships xmlns="${OPC_RELATIONSHIPS}">${rels.join('')}</Relationships>`));
    changes.set('[Content_Types].xml',encoder.encode(types));
    const bytes=target.parsed.archive.edit(changes),verified=readPptxThemeTarget(bytes,slideId);
    if(verified.context.themePart!==path||verified.master.part!==target.master.part)throw TypeError('Imported theme graph changed target ownership');
    return {bytes,name:source.name,changedParts:[...changes.keys()],mediaCount:resources.size};
}
inspectPresentationThemePackage.controlFactory=Object.freeze({topics:['documents','drawing'],constructor:'inspectPresentationThemePackage(bytes)',methods:'Read the primary THMX theme or first presentation master theme, with bounded CRC-verified image resources. No conversion, mutation or external fetch.'});
importPptxTheme.controlFactory=Object.freeze({topics:['documents','drawing'],constructor:'importPptxTheme(targetBytes,themeBytes,slideId)',methods:'Create a detached PowerPoint source candidate with a fresh master theme and remapped original media. Preserves unrelated masters, slides and source parts; signed/ambiguous packages reject. Caller must validate native import and commit through the document transaction owner.'});
