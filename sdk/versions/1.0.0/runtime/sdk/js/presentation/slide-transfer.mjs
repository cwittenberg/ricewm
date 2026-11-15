import {ZipParts} from '../zip-parts.mjs';
import {OpcPackage,OPC_CONTENT_TYPES,OPC_RELATIONSHIPS,opcRelationshipsPart,opcPartTarget} from '../opc-package.mjs';
import {parseXmlSource,patchXmlSource,escapeXmlAttribute} from '../xml-source.mjs';
import {readPptxPackage} from './slide-package.mjs';
import {preparePptxSlideStructure,inspectPptxSlideReferences} from './slide-references.mjs';
import {failure} from './contract-values.mjs';
import {fitDrawingmlPart} from '../drawingml-scale.mjs';

import {SLIDE_CLIPBOARD_BYTES} from './slide-transfer-contract.mjs';
export {SLIDE_CLIPBOARD_BYTES,SLIDE_CLIPBOARD_MIME} from './slide-transfer-contract.mjs';
const META='ricewm-slide-clipboard.json',SCHEMA='ricewm.presentation-slides/v1';
const P='http://schemas.openxmlformats.org/presentationml/2006/main',SP='http://purl.oclc.org/ooxml/presentationml/main';
const R='http://schemas.openxmlformats.org/officeDocument/2006/relationships',SR='http://purl.oclc.org/ooxml/officeDocument/relationships';
const A='http://schemas.openxmlformats.org/drawingml/2006/main',SA='http://purl.oclc.org/ooxml/drawingml/main';
const S='http://schemas.microsoft.com/office/powerpoint/2010/main';
const SLIDE='application/vnd.openxmlformats-officedocument.presentationml.slide+xml';
const LAYOUT='application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml';
const MASTER='application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml';
const NOTES_MASTER='application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml';
const NOTES='application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml';
const isRelationships=part=>/(?:^|\/)_rels\/[^/]*\.rels$/iu.test(part);
const decoder=new TextDecoder('utf-8',{fatal:true}),encoder=new TextEncoder();
const attr=(node,name,ns='')=>node.attributes.find(a=>a.localName===name&&a.namespaceURI===ns);
const fail=message=>{throw failure('INVALID_CLIPBOARD',message);};
const id=value=>typeof value==='string'&&/^pptx:[1-9]\d{2,9}$/u.test(value)&&Number(value.slice(5))>=256&&Number(value.slice(5))<=2147483647;
const children=(node,ns,name)=>node.children.filter(n=>n.namespaceURI===ns&&n.localName===name);
function attributes(xml,node,values) {
    const edits=[];
    for(const[name,value]of Object.entries(values)) {
        const old=attr(node,name);if(old)edits.push({start:old.valueStart,end:old.valueEnd,text:escapeXmlAttribute(value)});
        else edits.push({start:node.start+1+node.name.length,end:node.start+1+node.name.length,text:` ${name}="${escapeXmlAttribute(value)}"`});
    }
    return edits;
}
function append(xml,node,text) {return node.selfClosing?xml.slice(node.start,node.openEnd-2)+'>'+text+`</${node.name}>`:xml.slice(node.start,node.closeStart)+text+xml.slice(node.closeStart,node.end);}
const types=parts=>`<Types xmlns="${OPC_CONTENT_TYPES}">${[...parts].map(([name,type])=>`<Override PartName="${escapeXmlAttribute(opcPartTarget(name))}" ContentType="${escapeXmlAttribute(type)}"/>`).join('')}</Types>`;
const profile=root=>root.namespaceURI===P?{p:P,r:R,a:A}:root.namespaceURI===SP?{p:SP,r:SR,a:SA}:fail('Unsupported PowerPoint namespace');
function slideParts(parsed,opc) {
    const root=parseXmlSource(parsed.result.xml).root,{r}=profile(root),relationships=new Map(opc.relationships(parsed.path).map(rel=>[rel.id,rel]));
    return parsed.result.slides.map(slide=>{const rel=relationships.get(slide.relationship);if(!rel?.part||rel.type!==r+'/slide'||opc.contentType(rel.part)!==SLIDE)fail('Slide relationship and content type disagree');return {...slide,part:rel.part};});
}
function size(root,p) {
    const values=children(root,p,'sldSz');if(values.length!==1)fail('Missing presentation size');
    const result={cx:Number(attr(values[0],'cx')?.value),cy:Number(attr(values[0],'cy')?.value)};
    if(!Object.values(result).every(v=>Number.isSafeInteger(v)&&v>0&&v<=0x7fffffff))fail('Invalid presentation size');return result;
}
function checkedMetadata(meta) {
    if(!meta||Object.keys(meta).sort().join(',')!=='identity,links,profile,schema,size,slides'||meta.schema!==SCHEMA||![P,SP].includes(meta.profile)||meta.identity!==null&&(typeof meta.identity!=='string'||!/^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/iu.test(meta.identity))||!meta.size||Object.keys(meta.size).sort().join(',')!=='cx,cy'||!Object.values(meta.size).every(v=>Number.isSafeInteger(v)&&v>0&&v<=0x7fffffff)||!Array.isArray(meta.slides)||!meta.slides.length||meta.slides.length>32||!Array.isArray(meta.links)||meta.links.length>16384)fail('Invalid slide clipboard descriptor');
    const ids=new Set(),parts=new Set();
    for(const s of meta.slides){if(!s||Object.keys(s).sort().join(',')!=='id,part'||!id(s.id)||typeof s.part!=='string'||ids.has(s.id)||parts.has(s.part.toLowerCase()))fail('Invalid clipboard slide identity');ids.add(s.id);parts.add(s.part.toLowerCase());}
    const links=new Set();for(const link of meta.links){if(!link||Object.keys(link).sort().join(',')!=='id,owner,slide'||typeof link.owner!=='string'||typeof link.id!=='string'||!link.id||link.id.length>4096||!id(link.slide))fail('Invalid clipboard slide link');const key=JSON.stringify([link.owner,link.id]);if(links.has(key))fail('Duplicate clipboard slide link');links.add(key);}
    for(const row of [...meta.slides,...meta.links])Object.freeze(row);Object.freeze(meta.slides);Object.freeze(meta.links);Object.freeze(meta.size);return Object.freeze(meta);
}
function checkLinkOwner(opc,owner,rid,a,r) {
    const document=opc.xml(owner),nodes=document.nodes.filter(n=>n.attributes.some(attribute=>attribute.namespaceURI===r&&attribute.value===rid));
    if(nodes.some(n=>n.namespaceURI!==a||!['hlinkClick','hlinkHover'].includes(n.localName)||attr(n,'action')?.value!=='ppaction://hlinksldjump'))fail('An unselected slide has an unsupported incoming reference');
    return nodes;
}

/** A clipboard contains selected slide graphs, not a copy of the whole source
 * presentation. Nothing is written to the source document or clipboard host. */
export function createPptxSlideClipboard(input,selected,{identity=null}={}) {
    if(!Array.isArray(selected)||!selected.length||selected.length>32||new Set(selected).size!==selected.length||!selected.every(id))fail('Choose 1 to 32 distinct PowerPoint slides');
    const parsed=readPptxPackage(input),opc=new OpcPackage(parsed.archive),root=parseXmlSource(parsed.result.xml).root,{p,r,a}=profile(root);
    if(opc.hasSignatures)fail('Signed presentations retain their source extraction policy');
    const all=slideParts(parsed,opc),wanted=new Set(selected),slides=all.filter(s=>wanted.has(s.id));if(slides.length!==wanted.size)fail('Unknown clipboard slide');
    const byPart=new Map(all.map(s=>[s.part,s])),selectedParts=new Set(slides.map(s=>s.part));
    const parts=opc.closure(slides.map(s=>s.part),{include:rel=>{
        if(rel.part===parsed.path)fail('A selected resource depends on the whole presentation');
        return !byPart.has(rel.part)||selectedParts.has(rel.part);
    }});
    const data=new Map(),contentTypes=new Map(),links=[];
    for(const name of parts){data.set(name,opc.read(name));contentTypes.set(name,opc.contentType(name));}
    for(const owner of parts.filter(name=>!isRelationships(name))) {
        const relationships=opc.relationships(owner),edits=[],xml=opc.has(opcRelationshipsPart(owner))?decoder.decode(opc.read(opcRelationshipsPart(owner))):'';
        for(const rel of relationships)if(rel.part&&byPart.has(rel.part)) {
            if(rel.type!==r+'/slide')fail('Unexpected reference to a presentation slide');
            links.push({owner,id:rel.id,slide:byPart.get(rel.part).id});
            if(!selectedParts.has(rel.part)) {checkLinkOwner(opc,owner,rel.id,a,r);edits.push(...attributes(xml,rel.node,{Target:'ricewm-clipboard-slide:'+byPart.get(rel.part).id,TargetMode:'External'}));}
        }
        if(edits.length)data.set(opc.canonical(opcRelationshipsPart(owner)),encoder.encode(patchXmlSource(xml,edits)));
    }
    // Global styles, comment authors and embedded typefaces cannot be silently
    // detached. Their specialized merge paths must qualify before extraction.
    for(const owner of parts.filter(name=>!isRelationships(name))) {
        const type=opc.contentType(owner);
        if(/comments|commentAuthors|person\+xml/iu.test(type))fail('This selection requires a comment-author merge');
        if(type===NOTES&&opc.relationships(owner).some(rel=>rel.type===r+'/slide'&&(!rel.part||!selectedParts.has(rel.part))))fail('A notes part has an inconsistent slide owner');
        if(type===SLIDE&&opc.xml(owner).nodes.some(n=>n.namespaceURI===a&&n.localName==='tbl')) {
            const styles=opc.relationships(parsed.path).filter(rel=>rel.type===r+'/tableStyles');
            if(styles.some(rel=>opc.xml(rel.part).root.children.length))fail('This selection requires a custom table-style merge');
        }
    }
    if(children(root,p,'embeddedFontLst').some(node=>node.children.length))fail('This selection requires an embedded-font merge');
    const meta=checkedMetadata({schema:SCHEMA,identity,profile:p,size:size(root,p),slides:slides.map(({id,part})=>({id,part})),links});
    if(data.has(META)||data.has('[Content_Types].xml'))fail('Clipboard metadata part collision');
    data.set(META,encoder.encode(JSON.stringify(meta)));contentTypes.set(META,'application/vnd.ricewm.presentation-slides+json');
    data.set('[Content_Types].xml',encoder.encode(types(contentTypes)));
    const bytes=ZipParts.create(data,{maximumBytes:SLIDE_CLIPBOARD_BYTES,maximumPartBytes:SLIDE_CLIPBOARD_BYTES,maximumExpandedBytes:64*1024*1024});
    return Object.freeze({bytes,slideCount:slides.length});
}

export function inspectPptxSlideClipboard(input) {
    if(!(input instanceof Uint8Array)||!input.length||input.length>SLIDE_CLIPBOARD_BYTES)fail('Slide clipboard exceeds 4 MiB');
    const opc=new OpcPackage(input,{maximumBytes:SLIDE_CLIPBOARD_BYTES,maximumPartBytes:SLIDE_CLIPBOARD_BYTES});
    if(opc.hasSignatures)fail('Signed clipboard packages are unsupported');
    if(opc.contentType(META)!=='application/vnd.ricewm.presentation-slides+json')fail('Invalid clipboard metadata content type');
    let meta;try{meta=checkedMetadata(JSON.parse(decoder.decode(opc.read(META))));}catch(error){if(error.code)throw error;fail('Invalid clipboard metadata');}
    const {r,a}=profile({namespaceURI:meta.profile});
    for(const slide of meta.slides)if(opc.contentType(slide.part)!==SLIDE||opc.xml(slide.part).root.namespaceURI!==meta.profile||opc.xml(slide.part).root.localName!=='sld')fail('Clipboard slide profile disagrees with its descriptor');
    const parts=opc.closure(meta.slides.map(s=>s.part)),expected=new Set([...parts,META,'[Content_Types].xml']);
    if(opc.archive.names.some(name=>!expected.has(name)))fail('Clipboard includes unrelated package parts');
    const byPart=new Map(meta.slides.map(s=>[s.part,s.id])),linked=new Map(meta.links.map(link=>[JSON.stringify([link.owner,link.id]),link]));let found=0;
    for(const owner of parts.filter(name=>!isRelationships(name)))if(/comments|commentAuthors|person\+xml/iu.test(opc.contentType(owner)))fail('This clipboard requires a comment-author merge');
    for(const owner of parts.filter(name=>!isRelationships(name)))for(const rel of opc.relationships(owner)) {
        const link=linked.get(JSON.stringify([owner,rel.id]));
        if(link){found++;if(rel.type!==r+'/slide'||(rel.part?byPart.get(rel.part)!==link.slide:rel.target!=='ricewm-clipboard-slide:'+link.slide))fail('Clipboard slide link disagrees with its resource graph');if(!rel.part)checkLinkOwner(opc,owner,rel.id,a,r);}
        else if(rel.target.startsWith('ricewm-clipboard-slide:')||rel.part&&byPart.has(rel.part))fail('Missing clipboard slide link descriptor');
    }
    if(found!==meta.links.length)fail('Unknown clipboard link owner');
    return Object.freeze({opc,metadata:meta,parts,slideCount:meta.slides.length});
}

/** Assemble a disposable destination candidate. Resource relationships, source
 * layouts/masters, notes and internal slide links are remapped together. The
 * owner must still import/project this candidate before committing history. */
export function insertPptxSlideClipboard(destination,clipboard,{after=null,identity=null,preservedSlides=[],nonce=crypto.randomUUID().replaceAll('-','')}={}) {
    if(after!==null&&!id(after)||typeof nonce!=='string'||!/^[\da-f]{32}$/iu.test(nonce))fail('Invalid paste destination');
    const parsed=readPptxPackage(destination),target=new OpcPackage(parsed.archive),clip=inspectPptxSlideClipboard(clipboard),source=clip.opc,meta=clip.metadata;
    const structure=preparePptxSlideStructure(parsed.result.xml),document=parseXmlSource(structure.xml),root=document.root,{p,r,a}=profile(root);
    if(p!==meta.profile)fail('The clipboard and destination use different OOXML dialects');
    if(target.hasSignatures)fail('Signed presentations retain their native edit policy');
    const destinationSize=children(root,p,'sldSz').length?size(root,p):meta.size;
    const fitting={sourceSize:{width:meta.size.cx,height:meta.size.cy},destinationSize:{width:destinationSize.cx,height:destinationSize.cy}};
    const original=slideParts(parsed,target),byId=new Map(original.map(s=>[s.id,s]));
    const at=after===null?0:original.findIndex(s=>s.id===after)+1;if(after!==null&&!byId.has(after))fail('The paste destination slide no longer exists');
    if(original.length+meta.slides.length>4096)fail('Pasting would exceed the presentation slide limit');
    const prefix='ricewm-clipboard/'+nonce+'/',mapping=new Map(),changes=new Map(),newTypes=new Map();
    if(target.archive.names.some(name=>name.toLowerCase().startsWith(prefix.toLowerCase())))fail('Paste resource identity collision');
    let sequence=0;
    for(const part of clip.parts.filter(part=>!isRelationships(part))) {
        const suffix=part.slice(part.lastIndexOf('/')+1).split('.').slice(1).at(-1),type=source.contentType(part);
        const extension=suffix&&/^[a-z0-9]{1,32}$/iu.test(suffix)?suffix:/\+xml$|\/xml$/iu.test(type)?'xml':'bin';
        mapping.set(part,prefix+'part'+sequence+++'.'+extension);
    }
    const masterParts=clip.parts.filter(part=>!isRelationships(part)&&source.contentType(part)===MASTER);
    const notesMasters=clip.parts.filter(part=>!isRelationships(part)&&source.contentType(part)===NOTES_MASTER);
    if(notesMasters.length>1)fail('Multiple source notes masters need an explicit merge');
    const mainRelationships=target.relationships(parsed.path),mainById=new Map(mainRelationships.map(rel=>[rel.id,rel]));
    const one=name=>{const nodes=children(root,p,name);if(nodes.length>1)fail('Ambiguous destination '+name);return nodes[0]||null;};
    const masterList=one('sldMasterIdLst'),usedDefinitions=new Set((masterList?children(masterList,p,'sldMasterId'):[]).map(node=>Number(attr(node,'id')?.value)));
    for(const rel of mainRelationships)if(rel.type===r+'/slideMaster'&&rel.part)for(const node of target.xml(rel.part).nodes)if(node.namespaceURI===p&&node.localName==='sldLayoutId')usedDefinitions.add(Number(attr(node,'id')?.value));
    if([...usedDefinitions].some(value=>!Number.isSafeInteger(value)||value<2147483648||value>0xffffffff))fail('Invalid destination master/layout identity');
    let definition=2147483648;
    function definitionId(){while(usedDefinitions.has(definition))definition++;if(definition>0xffffffff)fail('No remaining master/layout identities');usedDefinitions.add(definition);return definition++;}
    const notesList=one('notesMasterIdLst'),notesEntries=notesList?children(notesList,p,'notesMasterId'):[];
    if(notesEntries.length>1)fail('Ambiguous destination notes master');
    const existingNotes=notesEntries.length?mainById.get(attr(notesEntries[0],'id',r)?.value):null;
    if(notesEntries.length&&(!existingNotes?.part||existingNotes.type!==r+'/notesMaster'||target.contentType(existingNotes.part)!==NOTES_MASTER))fail('Invalid destination notes-master relationship');
    const reused=new Set();
    if(notesMasters.length&&existingNotes){mapping.set(notesMasters[0],existingNotes.part);reused.add(notesMasters[0]);}
    let next=Math.max(255,...original.map(s=>Number(s.id.slice(5))));
    const copies=meta.slides.map(s=>{if(++next>2147483647)fail('No remaining slide identities');return {...s,created:'pptx:'+next};});
    const copiedById=new Map(copies.map(s=>[s.id,s])),links=new Map(meta.links.map(link=>[JSON.stringify([link.owner,link.id]),link]));
    if(!Array.isArray(preservedSlides)||!preservedSlides.every(id))fail('Invalid original slide identities');
    const preserved=new Set(preservedSlides),sameDocument=identity!==null&&meta.identity===identity;
    const mainAdds=[],usedRelationshipIds=new Set(mainRelationships.map(rel=>rel.id));
    function relationship(type,part) {
        let rid='riceClipboard'+nonce+'r'+mainAdds.length;while(usedRelationshipIds.has(rid))rid+='x';usedRelationshipIds.add(rid);
        mainAdds.push(`<Relationship xmlns="${OPC_RELATIONSHIPS}" Id="${rid}" Type="${r}/${type}" Target="${escapeXmlAttribute(opcPartTarget(part))}"/>`);return rid;
    }
    for(const owner of clip.parts.filter(part=>!isRelationships(part)&&!reused.has(part))) {
        const path=mapping.get(owner),type=source.contentType(owner);let body=source.read(owner);
        const relationships=source.relationships(owner),relPath=opcRelationshipsPart(owner),edits=[],removed=[];
        const relXml=source.has(relPath)?decoder.decode(source.read(relPath)):'';
        for(const rel of relationships) {
            const link=links.get(JSON.stringify([owner,rel.id]));let to;
            if(link) {
                to=type!==NOTES&&sameDocument&&preserved.has(link.slide)&&byId.has(link.slide)?byId.get(link.slide).part:copiedById.has(link.slide)?mapping.get(copiedById.get(link.slide).part):null;
                if(!to){removed.push(...checkLinkOwner(source,owner,rel.id,a,r));edits.push({start:rel.node.start,end:rel.node.end,text:''});continue;}
            } else if(rel.part)to=mapping.get(rel.part);
            else continue; // ordinary external links stay descriptors, never fetched
            if(!to)fail('Clipboard relationship is outside its selected graph');
            edits.push(...attributes(relXml,rel.node,{Target:opcPartTarget(to),TargetMode:'Internal'}));
        }
        if(removed.length)body=encoder.encode(patchXmlSource(decoder.decode(body),[...new Map(removed.map(node=>[node.start,node])).values()].map(node=>({start:node.start,end:node.end,text:''}))));
        if(type===MASTER) {
            const xml=decoder.decode(body),document=parseXmlSource(xml),edits=[];
            for(const node of document.nodes)if(node.namespaceURI===p&&node.localName==='sldLayoutId') {
                const previous=Number(attr(node,'id')?.value);if(!Number.isSafeInteger(previous)||previous<2147483648||previous>0xffffffff)fail('Invalid source layout identity');
                edits.push(...attributes(xml,node,{id:String(definitionId())}));
            }
            if(edits.length)body=encoder.encode(patchXmlSource(xml,edits));
        }
        if([SLIDE,LAYOUT,MASTER].includes(type))body=encoder.encode(fitDrawingmlPart(decoder.decode(body),fitting).xml);
        else if(/(?:drawingml\.(?:chart|diagramDrawing)|officedocument\.theme(?:Override)?)\+xml$/u.test(type))body=encoder.encode(fitDrawingmlPart(decoder.decode(body),{...fitting,resource:true}).xml);
        changes.set(path,body);newTypes.set(path,type);
        if(source.has(relPath)) {
            const path=opcRelationshipsPart(mapping.get(owner));changes.set(path,encoder.encode(patchXmlSource(relXml,edits)));newTypes.set(path,'application/vnd.openxmlformats-package.relationships+xml');
        }
    }
    const rootEdits=[],insertions=new Map();
    const ranks=new Map(['sldMasterIdLst','notesMasterIdLst','handoutMasterIdLst','sldIdLst','sldSz','notesSz','smartTags','embeddedFontLst','custShowLst','photoAlbum','custDataLst','kinsoku','defaultTextStyle','modifyVerifier','extLst'].map((name,index)=>[name,index]));
    function insertRoot(name,text) {
        const before=root.children.find(node=>node.namespaceURI===p&&(ranks.get(node.localName)??Infinity)>ranks.get(name));
        const at=before?.start??root.closeStart;
        if(root.selfClosing)fail('The destination lacks a presentation-size model');
        const records=insertions.get(at)||[];records.push({rank:ranks.get(name),text});insertions.set(at,records);
    }
    function addList(name,itemName,records) {
        if(!records.length)return;
        const list=one(name),text=records.map(record=>`<cp:${itemName} xmlns:cp="${p}" xmlns:cr="${r}"${record.id===undefined?'':` id="${record.id}"`} cr:id="${record.relationship}"/>`).join('');
        if(list)rootEdits.push({start:list.start,end:list.end,text:append(structure.xml,list,text)});
        else insertRoot(name,`<cp:${name} xmlns:cp="${p}">${text}</cp:${name}>`);
    }
    addList('sldMasterIdLst','sldMasterId',masterParts.map(part=>({id:definitionId(),relationship:relationship('slideMaster',mapping.get(part))})));
    if(notesMasters.length&&!existingNotes)addList('notesMasterIdLst','notesMasterId',[{relationship:relationship('notesMaster',mapping.get(notesMasters[0]))}]);
    const slideNodes=copies.map(copy=>`<cp:sldId xmlns:cp="${p}" xmlns:cr="${r}" id="${copy.created.slice(5)}" cr:id="${relationship('slide',mapping.get(copy.part))}"/>`).join('');
    const list=one('sldIdLst');
    if(list) {
        const before=children(list,p,'sldId')[at];
        if(before)rootEdits.push({start:before.start,end:before.start,text:slideNodes});
        else rootEdits.push({start:list.start,end:list.end,text:append(structure.xml,list,slideNodes)});
    } else insertRoot('sldIdLst',`<cp:sldIdLst xmlns:cp="${p}">${slideNodes}</cp:sldIdLst>`);
    if(!one('sldSz'))insertRoot('sldSz',`<cp:sldSz xmlns:cp="${p}" cx="${meta.size.cx}" cy="${meta.size.cy}"/>`);
    for(const[position,records]of insertions) {
        const text=records.sort((a,b)=>a.rank-b.rank).map(r=>r.text).join(''),sameStart=rootEdits.find(edit=>edit.start===position);
        if(sameStart)sameStart.text=text+sameStart.text;else rootEdits.push({start:position,end:position,text});
    }
    const mainXml=patchXmlSource(structure.xml,rootEdits),commands=copies.map((copy,index)=>({type:'insert-slide',after:index?copies[index-1].created:after}));
    changes.set(parsed.path,encoder.encode(structure.restore(mainXml,commands,copies.map(s=>s.created)).xml));
    const mainRelPath=opcRelationshipsPart(parsed.path);
    if(target.has(mainRelPath)){const xml=decoder.decode(target.read(mainRelPath)),document=parseXmlSource(xml);changes.set(target.canonical(mainRelPath),encoder.encode(append(xml,document.root,mainAdds.join(''))));}
    else{changes.set(mainRelPath,encoder.encode(`<Relationships xmlns="${OPC_RELATIONSHIPS}">${mainAdds.join('')}</Relationships>`));newTypes.set(mainRelPath,'application/vnd.openxmlformats-package.relationships+xml');}
    const content=decoder.decode(target.read('[Content_Types].xml')),contentRoot=parseXmlSource(content).root;
    const overrides=[...newTypes].map(([part,type])=>`<Override xmlns="${OPC_CONTENT_TYPES}" PartName="${escapeXmlAttribute(opcPartTarget(part))}" ContentType="${escapeXmlAttribute(type)}"/>`).join('');
    changes.set(target.canonical('[Content_Types].xml'),encoder.encode(append(content,contentRoot,overrides)));
    const bytes=target.archive.edit(changes),references=inspectPptxSlideReferences(decoder.decode(new OpcPackage(bytes).read(parsed.path)));
    const expected=[...original.slice(0,at).map(s=>s.id),...copies.map(s=>s.created),...original.slice(at).map(s=>s.id)];
    if(references.slides.length!==expected.length||references.slides.some((s,index)=>s.id!==expected[index]))fail('Pasted slide order is inconsistent');
    return Object.freeze({bytes,createdSlides:Object.freeze(copies.map(s=>s.created)),slideCount:references.slides.length});
}
createPptxSlideClipboard.controlFactory=Object.freeze({topics:['documents','workflow'],constructor:'createPptxSlideClipboard(bytes,slideIds,{identity:null})',methods:'Extract 1–32 selected PowerPoint slide dependency graphs into a fresh bounded typed clipboard ZIP. Retains editable content, owned notes and layouts; excludes unselected slides and unrelated parts. No clipboard host writes or document mutation. Unsupported global-resource merges reject explicitly.'});
inspectPptxSlideClipboard.controlFactory=Object.freeze({topics:['documents','workflow'],constructor:'inspectPptxSlideClipboard(bytes)',methods:'Validate bounded clipboard metadata, selected slides, dependency closure, relationships and source policy before staging a destination.'});
insertPptxSlideClipboard.controlFactory=Object.freeze({topics:['documents','workflow'],constructor:'insertPptxSlideClipboard(destination,clipboard,{after:null,identity:null,preservedSlides:[]})',methods:'Build an uncommitted package candidate with fresh slide/master/layout identities, owned notes, remapped resources and sections. Original hyperlink targets are preserved only for caller-verified live slide incarnations in preservedSlides and matching clipboard identity. Import/project before committing through BinaryDocumentHistory. Returns bytes and createdSlides. Does not save or mutate the destination.'});
