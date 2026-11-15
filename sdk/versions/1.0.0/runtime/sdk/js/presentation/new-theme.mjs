import {DEFAULT_THEME_FORMAT,presentationThemeXml} from './theme-markup.mjs';
import {readPptxPackage} from './slide-package.mjs';
import {OpcPackage,opcRelationshipsPart,OPC_RELATIONSHIPS,OPC_CONTENT_TYPES} from '../opc-package.mjs';
import {parseXmlSource,patchXmlSource,escapeXmlAttribute} from '../xml-source.mjs';

const A='http://schemas.openxmlformats.org/drawingml/2006/main',P='http://schemas.openxmlformats.org/presentationml/2006/main',R='http://schemas.openxmlformats.org/officeDocument/2006/relationships';
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const attr=(n,k)=>n.attributes.find(a=>!a.namespaceURI&&a.localName===k)?.value;
const slots=['dk1','lt1','dk2','lt2','accent1','accent2','accent3','accent4','accent5','accent6','hlink','folHlink'];
export const DEFAULT_PRESENTATION_THEME=Object.freeze({name:'Rice',majorFont:'Inter',minorFont:'Inter',colors:Object.freeze(['000000','FFFFFF','1E2B3C','EDEBE7','2E6FD8','F26B1D','2EA36F','8A5CD8','D83F6B','E3B21C','2E6FD8','7A5BA6'])});
function themeXml(theme) {
    if(!theme||typeof theme.name!=='string'||!Array.isArray(theme.colors)||theme.colors.length!==12)throw TypeError('Invalid new presentation theme');
    return presentationThemeXml({name:theme.name,colorsName:theme.name,colors:theme.colors.map(v=>'#'+v),
        fonts:{name:theme.name,major:{latin:theme.majorFont},minor:{latin:theme.minorFont}},format:{...DEFAULT_THEME_FORMAT,name:theme.name}});
}

function append(xml,namespace,name,text) {
    const root=parseXmlSource(xml).root;
    if(root.namespaceURI!==namespace||root.localName!==name||root.selfClosing)throw TypeError('Invalid new presentation template part');
    return patchXmlSource(xml,[{start:root.closeStart,end:root.closeStart,text}]);
}
/** Initialize an explicitly new source template before its history is attached.
 * Existing imported files never call this path. A template that already owns a
 * theme is retained. Signatures and OPC identities retain their ordinary guards. */
export function initializePptxTheme(input,theme=DEFAULT_PRESENTATION_THEME) {
    const original=readPptxPackage(input),opc=new OpcPackage(original.archive);
    const presentation=opc.xml(original.path).root;
    if(presentation.namespaceURI!==P||presentation.localName!=='presentation'||original.result.slides.length!==1)throw TypeError('Theme initialization requires a new one-slide PowerPoint template');
    const masters=opc.relationships(original.path).filter(rel=>rel.type===R+'/slideMaster');
    if(masters.length!==1||!masters[0].part)throw TypeError('New template requires one internal slide master');
    const master=masters[0].part,relationships=opc.relationships(master);
    if(relationships.some(rel=>rel.type===R+'/theme'))return {bytes:input.slice(),changedParts:[]};
    const path='ppt/theme/rice-theme.xml';if(opc.has(path))throw TypeError('New template theme path already exists');
    const ids=new Set(relationships.map(r=>r.id));let serial=1;while(ids.has('riceTheme'+serial))serial++;
    const relPath=opcRelationshipsPart(master),relXml=decoder.decode(opc.read(relPath));
    const edits=new Map([[path,encoder.encode(themeXml(theme))],
        [relPath,encoder.encode(append(relXml,OPC_RELATIONSHIPS,'Relationships',`<Relationship xmlns="${OPC_RELATIONSHIPS}" Id="riceTheme${serial}" Type="${R}/theme" Target="../theme/rice-theme.xml"/>`))],
        ['[Content_Types].xml',encoder.encode(append(decoder.decode(opc.read('[Content_Types].xml')),OPC_CONTENT_TYPES,'Types',`<Override xmlns="${OPC_CONTENT_TYPES}" PartName="/${path}" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>`))]]);
    // The retained template's initial placeholder text has explicit Arial. Tie
    // only its newly created placeholders to the actual theme font references.
    const slideRel=opc.relationships(original.path).find(rel=>rel.id===original.result.slides[0].relationship),slide=slideRel?.part;
    if(!slide)throw TypeError('New template slide identity is missing');
    const xml=decoder.decode(opc.read(slide)),document=parseXmlSource(xml),fontEdits=[];
    for(const shape of document.root.children.flatMap(n=>n.children).flatMap(n=>n.children).filter(n=>n.namespaceURI===P&&n.localName==='sp')){
        const placeholder=document.nodes.find(n=>n.start>=shape.start&&n.end<=shape.end&&n.namespaceURI===P&&n.localName==='ph');
        if(!placeholder)continue;
        const font=['title','ctrTitle'].includes(attr(placeholder,'type'))?'+mj-lt':'+mn-lt';
        for(const node of document.nodes.filter(n=>n.start>=shape.start&&n.end<=shape.end&&n.namespaceURI===A&&n.localName==='latin')){
            const value=node.attributes.find(a=>!a.namespaceURI&&a.localName==='typeface');
            if(value)fontEdits.push({start:value.valueStart,end:value.valueEnd,text:font});
        }
    }
    if(fontEdits.length)edits.set(slide,encoder.encode(patchXmlSource(xml,fontEdits)));
    return {bytes:original.archive.edit(edits),changedParts:[...edits.keys()]};
}
