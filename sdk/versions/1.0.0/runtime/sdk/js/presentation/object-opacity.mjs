import {readPptxPackage} from './slide-package.mjs';
import {OpcPackage} from '../opc-package.mjs';
import {parseXmlSource,patchXmlSource} from '../xml-source.mjs';
import {checkedObjectCommand} from './authoring-contract.mjs';

export const OBJECT_OPACITY_COMMANDS = new Set(['set-object-fill-opacity','set-object-line-opacity']);
const P='http://schemas.openxmlformats.org/presentationml/2006/main',SP='http://purl.oclc.org/ooxml/presentationml/main';
const A='http://schemas.openxmlformats.org/drawingml/2006/main',SA='http://purl.oclc.org/ooxml/drawingml/main';
const R='http://schemas.openxmlformats.org/officeDocument/2006/relationships',SR='http://purl.oclc.org/ooxml/officeDocument/relationships';
const decoder=new TextDecoder('utf-8',{fatal:true}),encoder=new TextEncoder();
const attr=(n,key)=>n.attributes.find(a=>a.localName===key&&!a.namespaceURI)?.value;
const children=(node,ns,name)=>node.children.filter(n=>n.namespaceURI===ns&&n.localName===name);
const fail=message=>{throw new TypeError('Object transparency: '+message);};
function slideSource(parsed,opc,slide) {
    const ref=parsed.result.slides.find(s=>s.id===slide);if(!ref)fail('unknown slide');
    const root=parseXmlSource(parsed.result.xml).root,r=root.namespaceURI===P?R:SR;
    const rel=opc.relationships(parsed.path).find(item=>item.id===ref.relationship);
    if(!rel?.part||rel.type!==r+'/slide'||opc.contentType(rel.part)!=='application/vnd.openxmlformats-officedocument.presentationml.slide+xml')fail('invalid slide relationship');
    return rel.part;
}
function paints(xml,slide) {
    const doc=parseXmlSource(xml),p=doc.root.namespaceURI,a=p===P?A:SA;
    if(![P,SP].includes(p)||doc.root.localName!=='sld')fail('invalid slide root');
    const identities=new Map(),result=new Map();
    for(const node of doc.nodes.filter(n=>n.namespaceURI===p&&n.localName==='cNvPr')) {
        const value=attr(node,'id');if(!/^\d{1,10}$/u.test(value||'')||Number(value)>0xffffffff)continue;
        const key=String(Number(value));if(identities.has(key))fail('ambiguous shape identity');identities.set(key,node);
    }
    function solid(parent) {
        if(!parent)return null;
        const fills=parent.children.filter(n=>n.namespaceURI===a&&['noFill','solidFill','gradFill','blipFill','pattFill','grpFill'].includes(n.localName));
        if(fills.length!==1||fills[0].localName!=='solidFill')return null;
        const colors=fills[0].children.filter(n=>n.namespaceURI===a&&['srgbClr','schemeClr','prstClr','sysClr','scrgbClr','hslClr'].includes(n.localName));
        if(colors.length!==1)return null;
        const color=colors[0],alphas=children(color,a,'alpha');
        if(alphas.length>1||color.children.some(n=>n.namespaceURI===a&&['alphaMod','alphaOff'].includes(n.localName)))return null;
        if(alphas.length&&(!/^\d{1,6}$/u.test(attr(alphas[0],'val')||'')||Number(attr(alphas[0],'val'))>100000))return null;
        return {color,alpha:alphas[0]||null,namespace:a};
    }
    for(const[key,node]of identities) {
        const nv=node.parent,shape=nv?.parent;
        if(!shape||shape.namespaceURI!==p||!['sp','cxnSp'].includes(shape.localName)||nv.namespaceURI!==p||nv.localName!==(shape.localName==='sp'?'nvSpPr':'nvCxnSpPr'))continue;
        const properties=children(shape,p,'spPr');if(properties.length!==1)continue;
        const lines=children(properties[0],a,'ln');
        result.set(`${slide}/shape:${key}`,{fill:solid(properties[0]),line:lines.length===1?solid(lines[0]):null});
    }
    return result;
}

/** Capabilities describe explicit understood solid paints only. Inherited,
 * gradient, picture, ambiguous and signed sources are never silently flattened. */
export function inspectPptxObjectOpacity(input,slide) {
    const parsed=readPptxPackage(input),opc=new OpcPackage(parsed.archive),part=slideSource(parsed,opc,slide);
    return new Map([...paints(decoder.decode(opc.read(part)),slide)].map(([id,value])=>[id,{fill:Boolean(value.fill),line:Boolean(value.line)}]));
}

/** Keep inherited backgrounds distinct from an explicitly authored solid fill.
 * The native resolved paint remains authoritative for rendering. */
export function inspectPptxBackgroundKind(input,slide) {
    const parsed=readPptxPackage(input),opc=new OpcPackage(parsed.archive),part=slideSource(parsed,opc,slide);
    const doc=parseXmlSource(decoder.decode(opc.read(part))),p=doc.root.namespaceURI,a=p===P?A:SA;
    const common=children(doc.root,p,'cSld');
    if(common.length!==1)return 'unknown';
    const backgrounds=children(common[0],p,'bg');
    if(!backgrounds.length)return 'layout';
    if(backgrounds.length!==1)return 'unknown';
    const properties=children(backgrounds[0],p,'bgPr');
    if(properties.length!==1)return 'unknown';
    const paints=properties[0].children.filter(n=>n.namespaceURI===a&&['solidFill','gradFill','blipFill','pattFill','noFill'].includes(n.localName));
    if(paints.length!==1)return 'unknown';
    return {solidFill:'solid',gradFill:'gradient',blipFill:'picture',pattFill:'pattern',noFill:'none'}[paints[0].localName];
}

/** Source-local alpha changes preserve color/theme transforms and every other
 * ZIP member. The document owner must import and atomically commit the result. */
export function editPptxObjectOpacity(input,commands) {
    if(!Array.isArray(commands)||!commands.length||commands.length>32)fail('expected 1–32 commands');
    const parsed=readPptxPackage(input),opc=new OpcPackage(parsed.archive),changes=new Map();
    for(const raw of commands) {
        const command=checkedObjectCommand(raw);if(!OBJECT_OPACITY_COMMANDS.has(command.type))fail('unsupported command');
        const part=slideSource(parsed,opc,command.slide),xml=changes.get(part)??decoder.decode(opc.read(part));
        const paint=paints(xml,command.slide).get(command.object)?.[command.type==='set-object-fill-opacity'?'fill':'line'];
        if(!paint)fail('select an editable explicit solid '+(command.type==='set-object-fill-opacity'?'fill':'line'));
        const alpha=`<rdpaint:alpha xmlns:rdpaint="${paint.namespace}" val="${Math.round(command.opacity*100000)}"/>`,color=paint.color;
        const edit=paint.alpha?{start:paint.alpha.start,end:paint.alpha.end,text:alpha}:color.selfClosing
            ?{start:color.openEnd-2,end:color.openEnd,text:`>${alpha}</${color.name}>`}
            :{start:color.closeStart,end:color.closeStart,text:alpha};
        changes.set(part,patchXmlSource(xml,[edit]));
    }
    const replacements=new Map([...changes].map(([part,xml])=>[part,encoder.encode(xml)]));
    return {bytes:parsed.archive.replace(replacements),changedParts:[...changes.keys()]};
}
