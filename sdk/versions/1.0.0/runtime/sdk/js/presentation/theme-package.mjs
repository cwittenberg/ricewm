import {readPptxPackage} from './slide-package.mjs';
import {OpcPackage,opcRelationshipsPart,opcPartTarget,OPC_RELATIONSHIPS,OPC_CONTENT_TYPES} from '../opc-package.mjs';
import {parseXmlSource,patchXmlSource,escapeXmlAttribute} from '../xml-source.mjs';
import {readPptxSlideTheme,drawingmlColorReference} from './drawingml-theme.mjs';
import {resolveColorReference} from '../color-references.mjs';
import {THEME_COLOR_SLOTS,checkedThemeCommand,checkedThemeFonts} from './theme-contract.mjs';
import {presentationThemeXml,presentationFontSchemeXml,presentationFormatSchemeXml} from './theme-markup.mjs';

const P='http://schemas.openxmlformats.org/presentationml/2006/main',SP='http://purl.oclc.org/ooxml/presentationml/main';
const R='http://schemas.openxmlformats.org/officeDocument/2006/relationships',SR='http://purl.oclc.org/ooxml/officeDocument/relationships';
const THEME_MIME='application/vnd.openxmlformats-officedocument.theme+xml';
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true}),esc=escapeXmlAttribute;
const attribute=(node,name)=>node.attributes.find(a=>!a.namespaceURI&&a.localName===name);
const attr=(node,name)=>attribute(node,name)?.value;
const one=(node,ns,name)=>{const found=node?.children.filter(c=>c.namespaceURI===ns&&c.localName===name)??[];if(found.length>1)throw TypeError('Ambiguous theme '+name);return found[0]??null;};
const text=(opc,part)=>decoder.decode(opc.read(part));
function themeName(node,fallback='Custom'){
    const value=(attr(node??{attributes:[]},'name')||fallback).trim()||fallback;
    if(value.length>256||/[\x00-\x1f\x7f]/u.test(value))throw TypeError('Unsupported theme name');
    return value;
}
function source(input,slideId){
    const parsed=readPptxPackage(input),opc=new OpcPackage(parsed.archive),slide=parsed.result.slides.find(s=>s.id===slideId);
    if(!slide)throw TypeError('Unknown presentation theme slide');
    const p=parseXmlSource(parsed.result.xml).root.namespaceURI,r=p===P?R:SR;
    if(![P,SP].includes(p))throw TypeError('Unsupported presentation theme namespace');
    const rel=opc.relationships(parsed.path).find(v=>v.id===slide.relationship);
    if(!rel?.part||rel.type!==r+'/slide')throw TypeError('Invalid theme slide identity');
    const context=readPptxSlideTheme(opc,rel.part,p),master=context.owners.find(v=>v.root.localName==='sldMaster');
    if(!master)throw TypeError('The slide has no editable master');
    const masters=opc.relationships(parsed.path).filter(v=>v.type===r+'/slideMaster'&&v.part===master.part);
    const list=one(parseXmlSource(parsed.result.xml).root,p,'sldMasterIdLst');
    if(masters.length!==1||!list?.children.some(n=>n.namespaceURI===p&&n.localName==='sldMasterId'&&n.attributes.some(a=>a.namespaceURI===r&&a.localName==='id'&&a.value===masters[0].id)))throw TypeError('Master is not an owned presentation master');
    return {parsed,opc,context,master,r,p};
}
function fontSet(node,a){
    if(!node)throw TypeError('Missing theme font set');
    const latin=one(node,a,'latin'),ea=one(node,a,'ea'),cs=one(node,a,'cs');
    return {latin:attr(latin??{attributes:[]},'typeface')??'',ea:attr(ea??{attributes:[]},'typeface')??'',cs:attr(cs??{attributes:[]},'typeface')??'',scripts:node.children.filter(n=>n.namespaceURI===a&&n.localName==='font').map(n=>({script:attr(n,'script'),typeface:attr(n,'typeface')}))};
}
function state(s){
    const {context:ctx}=s,a=ctx.a,root=ctx.themeRoot,elements=one(root,a,'themeElements'),colors=one(elements,a,'clrScheme'),fonts=one(elements,a,'fontScheme'),format=one(elements,a,'fmtScheme');
    if(!root)return null;
    // The Design scheme edits the stored master theme. Slide/layout overrides
    // remain separate and continue to affect the effective slide palette.
    const resolve=(slot,seen=new Set())=>{
        if(seen.has(slot)||seen.size>=12)throw TypeError('Cyclic theme color');seen=new Set(seen).add(slot);
        const entry=one(colors,a,slot);if(entry?.children.length!==1)throw TypeError('Incomplete theme color scheme');const ref=drawingmlColorReference(entry.children[0],a);if(!ref)throw TypeError('Unsupported theme color');
        return resolveColorReference(ref,next=>resolve(next,seen));
    };
    const values=THEME_COLOR_SLOTS.map(slot=>{const color=resolve(slot);if(color.opacity!==1)throw TypeError('Theme color is not opaque');return color.color;});
    const fontScheme=checkedThemeFonts({name:attr(fonts??{attributes:[]},'name')||'Custom',major:fontSet(one(fonts,a,'majorFont'),a),minor:fontSet(one(fonts,a,'minorFont'),a)});
    return {name:themeName(root),colorsName:themeName(colors),colors:values,fontName:fontScheme.name,majorFont:fontScheme.major.latin,minorFont:fontScheme.minor.latin,formatName:themeName(format),fonts:fontScheme,master:s.master.part,theme:ctx.themePart};
}
export function inspectPptxTheme(input,slideId){return state(source(input,slideId));}
export function inspectPptxThemeScope(input,slideId){const target=source(input,slideId);return {master:target.master.part,theme:target.context.themePart};}
function setAttribute(xml,node,key,value){
    const old=attribute(node,key),insert=node.openEnd-(node.selfClosing?2:1);
    return patchXmlSource(xml,[old?{start:old.valueStart,end:old.valueEnd,text:esc(value)}:{start:insert,end:insert,text:` ${key}="${esc(value)}"`}]);
}
function childXml(xml,parent,a,name,replacement){
    const old=one(parent,a,name);if(old)return patchXmlSource(xml,[{start:old.start,end:old.end,text:replacement}]);
    if(parent.selfClosing)throw TypeError('Incomplete theme container');
    return patchXmlSource(xml,[{start:parent.closeStart,end:parent.closeStart,text:replacement}]);
}
function append(xml,namespace,rootName,addition){
    const root=parseXmlSource(xml).root;if(root.namespaceURI!==namespace||root.localName!==rootName)throw TypeError('Invalid theme package metadata');
    if(root.selfClosing)return patchXmlSource(xml,[{start:root.openEnd-2,end:root.openEnd,text:`>${addition}</${root.name}>`}]);
    return patchXmlSource(xml,[{start:root.closeStart,end:root.closeStart,text:addition}]);
}
function replaceScheme(xml,a,kind,replacement){
    const root=parseXmlSource(xml).root,elements=one(root,a,'themeElements');if(!elements)throw TypeError('Missing theme elements');return childXml(xml,elements,a,kind,replacement);
}
function editColors(xml,a,command){
    let root=parseXmlSource(xml).root,scheme=one(one(root,a,'themeElements'),a,'clrScheme');if(!scheme)throw TypeError('Missing theme colors');
    xml=setAttribute(xml,scheme,'name',command.name??'Custom');root=parseXmlSource(xml).root;scheme=one(one(root,a,'themeElements'),a,'clrScheme');
    const edits=[];
    for(const [slot,color]of Object.entries(command.colors)){
        const node=one(scheme,a,slot),replacement=`<a:${slot} xmlns:a="${a}"><a:srgbClr val="${color.slice(1)}"/></a:${slot}>`;
        if(!node)throw TypeError('Incomplete theme colors');edits.push({start:node.start,end:node.end,text:replacement});
    }
    return patchXmlSource(xml,edits);
}
function editFonts(xml,a,command){
    if(command.fonts)return replaceScheme(xml,a,'fontScheme',presentationFontSchemeXml(command.fonts,a));
    let scheme=one(one(parseXmlSource(xml).root,a,'themeElements'),a,'fontScheme');if(!scheme)throw TypeError('Missing theme fonts');
    xml=setAttribute(xml,scheme,'name',command.name??'Custom');
    const faces={major:command.major,minor:command.minor??(command.name===undefined?command.major:undefined)};
    for(const [kind,face]of Object.entries(faces))if(face!==undefined){
        const root=parseXmlSource(xml).root;scheme=one(one(root,a,'themeElements'),a,'fontScheme');const set=one(scheme,a,kind+'Font'),latin=one(set,a,'latin');
        if(!latin)throw TypeError('Missing theme Latin font');xml=setAttribute(xml,latin,'typeface',face);
    }
    return xml;
}
function themeShared(s){
    const theme=s.context.themePart;if(!theme)return false;
    for(const path of s.opc.archive.names.filter(n=>n.endsWith('.rels'))){
        let owner='';if(path!=='_rels/.rels'){const at=path.lastIndexOf('_rels/');if(at<0)continue;owner=path.slice(0,at)+path.slice(at+6,-5);if(!s.opc.has(owner))throw TypeError('Orphan theme relationship part');}
        if(owner===s.master.part)continue;
        if(s.opc.relationships(owner).some(rel=>rel.part===theme&&rel.type===s.r+'/theme'))return true;
    }
    return false;
}
function installTheme(s,xml,changes,{forceNew=false,copyExistingRelationships=true}={}){
    const {opc,context:ctx,master,r}=s;
    if(ctx.themePart&&!forceNew&&!themeShared(s)){changes.set(ctx.themePart,encoder.encode(xml));return ctx.themePart;}
    const dir=ctx.themePart?.slice(0,ctx.themePart.lastIndexOf('/')+1)??'ppt/theme/';let i=1,path;
    do{path=dir+'rice-theme-'+i+++'.xml';}while(opc.has(path));
    changes.set(path,encoder.encode(xml));
    if(copyExistingRelationships&&ctx.themePart&&opc.has(opcRelationshipsPart(ctx.themePart)))changes.set(opcRelationshipsPart(path),opc.read(opcRelationshipsPart(ctx.themePart)));
    const relPart=opcRelationshipsPart(master.part),old=opc.has(relPart)?text(opc,relPart):`<Relationships xmlns="${OPC_RELATIONSHIPS}"/>`,doc=parseXmlSource(old),themeRels=doc.root.children.filter(n=>n.namespaceURI===OPC_RELATIONSHIPS&&n.localName==='Relationship'&&attr(n,'Type')===r+'/theme');
    if(themeRels.length>1)throw TypeError('Ambiguous master theme relationship');
    let next;
    if(themeRels.length)next=setAttribute(old,themeRels[0],'Target',opcPartTarget(path));
    else{const ids=new Set(doc.root.children.map(n=>attr(n,'Id')));let id=1;while(ids.has('riceTheme'+id))id++;next=append(old,OPC_RELATIONSHIPS,'Relationships',`<Relationship xmlns="${OPC_RELATIONSHIPS}" Id="riceTheme${id}" Type="${r}/theme" Target="${opcPartTarget(path)}"/>`);}
    changes.set(relPart,encoder.encode(next));
    changes.set('[Content_Types].xml',encoder.encode(append(text(opc,'[Content_Types].xml'),OPC_CONTENT_TYPES,'Types',`<Override xmlns="${OPC_CONTENT_TYPES}" PartName="${opcPartTarget(path)}" ContentType="${THEME_MIME}"/>`)));
    return path;
}
function replaceMasterIdentity(s,name){
    let xml=text(s.opc,s.master.part),root=parseXmlSource(xml).root,common=one(root,s.p,'cSld');if(!common)throw TypeError('Missing master common data');
    xml=setAttribute(xml,common,'name',name);root=parseXmlSource(xml).root;const map=one(root,s.p,'clrMap');
    const defaults={bg1:'lt1',tx1:'dk1',bg2:'lt2',tx2:'dk2',...Object.fromEntries(THEME_COLOR_SLOTS.slice(4).map(v=>[v,v]))};
    if(!map)throw TypeError('Missing master color map');
    // Whole-theme selection resets the master's mapping; partial color/font/
    // effect changes preserve it, along with all slide/layout overrides.
    for(const [key,value]of Object.entries(defaults)){root=parseXmlSource(xml).root;xml=setAttribute(xml,one(root,s.p,'clrMap'),key,value);}
    return xml;
}
/** Source candidate only. The document owner must import/project successfully
 * and commit through its revision-checked binary history before acknowledging. */
export function editPptxTheme(input,commands){
    if(!Array.isArray(commands)||!commands.length||commands.length>32)throw TypeError('Expected 1–32 theme commands');
    commands=commands.map(checkedThemeCommand);let bytes=input.slice();const changed=new Set();
    for(const command of commands){
        const s=source(bytes,command.slide),changes=new Map(),a=s.context.a;let xml;
        if(command.type==='set-presentation-theme'){
            xml=presentationThemeXml(command.theme,a);changes.set(s.master.part,encoder.encode(replaceMasterIdentity(s,command.theme.name)));
        }else{
            const current=state(s);if(!current)throw TypeError('Apply a complete theme before editing its components');xml=text(s.opc,s.context.themePart);
            if(command.type==='set-theme-colors')xml=editColors(xml,a,command);
            else if(command.type==='set-theme-fonts')xml=editFonts(xml,a,command);
            else xml=replaceScheme(xml,a,'fmtScheme',presentationFormatSchemeXml(command.format,a));
        }
        if(s.context.themePart&&xml===text(s.opc,s.context.themePart)&&!changes.size)continue;
        installTheme(s,xml,changes);for(const name of changes.keys())changed.add(name);bytes=s.parsed.archive.edit(changes);
        // The rewritten graph must still resolve the same owned master.
        const next=source(bytes,command.slide);if(next.master.part!==s.master.part)throw TypeError('Theme edit changed the master identity');
    }
    return {bytes,changedParts:[...changed]};
}
inspectPptxTheme.controlFactory=Object.freeze({topics:['documents','drawing'],constructor:'inspectPptxTheme(bytes,slideId)',methods:'Inspect the source slide master theme: colors, Latin/EA/complex-script fonts, per-script mappings and format name. Read-only; rejects signed or ambiguous source identity.'});
editPptxTheme.controlFactory=Object.freeze({topics:['documents','drawing'],constructor:'editPptxTheme(bytes,commands)',methods:'Atomic source candidates for complete themes, partial colors, font pairs and format matrices. Isolates shared theme parts, preserves unrelated masters/media/overrides and requires ordinary native validation plus the owning document history before commit.'});

// Shared source-policy internals for the separately bounded theme-file importer.
export {source as readPptxThemeTarget,installTheme,replaceMasterIdentity};
