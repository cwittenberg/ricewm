import {ZipParts} from '../zip-parts.mjs';
import {parseXmlSource,patchXmlSource,escapeXmlAttribute} from '../xml-source.mjs';

export const ODF=Object.freeze({office:'urn:oasis:names:tc:opendocument:xmlns:office:1.0',draw:'urn:oasis:names:tc:opendocument:xmlns:drawing:1.0',table:'urn:oasis:names:tc:opendocument:xmlns:table:1.0',text:'urn:oasis:names:tc:opendocument:xmlns:text:1.0',style:'urn:oasis:names:tc:opendocument:xmlns:style:1.0',fo:'urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0',svg:'urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0',manifest:'urn:oasis:names:tc:opendocument:xmlns:manifest:1.0',xml:'http://www.w3.org/XML/1998/namespace',xmlns:'http://www.w3.org/2000/xmlns/'});
export const odfFail=message=>{throw TypeError('OpenDocument text range: '+message);};
export const odfAttribute=(node,namespace,name)=>node.attributes.find(a=>a.namespaceURI===namespace&&a.localName===name)?.value;
export const odfChildren=(node,namespace,name)=>node.children.filter(n=>n.namespaceURI===namespace&&n.localName===name);
export const odfRaw=(source,node)=>source.slice(node.start,node.end);
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
export const odfBytes=value=>encoder.encode(value).length;
export const odfEscape=value=>escapeXmlAttribute(value).replaceAll('&quot;','"').replaceAll('&apos;',"'");
export function odfAppend(source,node,content){
    return node.selfClosing?patchXmlSource(source,[{start:node.openEnd-2,end:node.openEnd,text:'>'+content+`</${node.name}>`}]):patchXmlSource(source,[{start:node.closeStart,end:node.closeStart,text:content}]);
}
export function odfSetAttributes(source,node,values){
    const edits=[],missing=[],prefixes=new Map();let serial=0;
    for(const[namespace,name,value]of values){
        const present=node.attributes.find(a=>a.namespaceURI===namespace&&a.localName===name);
        if(present){edits.push({start:present.valueStart,end:present.valueEnd,text:escapeXmlAttribute(String(value))});continue;}
        let prefix='';
        if(namespace){
            prefix=Object.entries(node.namespaces).find(([prefix,uri])=>prefix&&uri===namespace)?.[0]||prefixes.get(namespace);
            if(!prefix){do{prefix='rdat'+(++serial);}while(Object.hasOwn(node.namespaces,prefix));prefixes.set(namespace,prefix);missing.push(` xmlns:${prefix}="${escapeXmlAttribute(namespace)}"`);}
        }
        missing.push(` ${prefix?prefix+':':''}${name}="${escapeXmlAttribute(String(value))}"`);
    }
    if(missing.length)edits.push({start:node.openEnd-(node.selfClosing?2:1),end:node.openEnd-(node.selfClosing?2:1),text:missing.join('')});
    return patchXmlSource(source,edits);
}
export function odfFragment(source,node){
    const declared=new Set(node.attributes.filter(a=>a.namespaceURI===ODF.xmlns).map(a=>a.name==='xmlns'?'':a.localName));
    const ns=Object.entries(node.namespaces).filter(([prefix])=>prefix!=='xml'&&!declared.has(prefix)).map(([prefix,uri])=>` xmlns${prefix?':'+prefix:''}="${escapeXmlAttribute(uri)}"`).join('');
    if(ns.length>32768)odfFail('namespace context exceeds source-edit bounds');
    const value=odfRaw(source,node),at=node.openEnd-node.start-(node.selfClosing?2:1);
    return value.slice(0,at)+ns+value.slice(at);
}
export function odfParseFragment(fragment,context){
    let prefix='rrwrap';while(Object.hasOwn(context.namespaces,prefix))prefix+='x';
    const declarations=Object.entries(context.namespaces).filter(([name])=>name!=='xml').map(([name,uri])=>` xmlns${name?':'+name:''}="${escapeXmlAttribute(uri)}"`).join('');
    if(declarations.length>32768)odfFail('namespace context exceeds source-edit bounds');
    const opening=`<${prefix}:root xmlns:${prefix}="urn:ricewm:odf-fragment"${declarations}>`,suffix=`</${prefix}:root>`,source=opening+fragment+suffix,doc=parseXmlSource(source);
    if(doc.root.children.length!==1)odfFail('ambiguous source fragment');return{source,node:doc.root.children[0],offset:opening.length,suffix};
}
export function readOdpSource(input){
    const archive=new ZipParts(input,{maximumBytes:8*1024*1024,maximumPartBytes:32*1024*1024,maximumExpandedBytes:128*1024*1024});
    if(archive.names.some(name=>{const n=name.toLowerCase();return n.startsWith('_xmlsignatures/')||n.startsWith('meta-inf/')&&n.split('/').at(-1).includes('signatures');}))odfFail('signed presentations require a signature-aware workflow');
    const mime=decoder.decode(archive.read('mimetype'));
    if(!['application/vnd.oasis.opendocument.presentation','application/vnd.oasis.opendocument.presentation-template'].includes(mime))odfFail('expected an ODP or OTP package');
    const manifest=parseXmlSource(decoder.decode(archive.read('META-INF/manifest.xml')));
    if(manifest.root.namespaceURI!==ODF.manifest||manifest.root.localName!=='manifest')odfFail('invalid manifest');
    if(manifest.nodes.some(n=>n.namespaceURI===ODF.manifest&&n.localName==='encryption-data'))odfFail('encrypted presentations are not editable');
    const entries=odfChildren(manifest.root,ODF.manifest,'file-entry'),roots=entries.filter(n=>odfAttribute(n,ODF.manifest,'full-path')==='/');
    if(roots.length!==1||odfAttribute(roots[0],ODF.manifest,'media-type')!==mime)odfFail('manifest and mimetype disagree');
    const source=decoder.decode(archive.read('content.xml')),document=parseXmlSource(source),root=document.root;
    if(root.namespaceURI!==ODF.office||root.localName!=='document-content')odfFail('invalid content root');
    const bodies=odfChildren(root,ODF.office,'body');if(bodies.length!==1||bodies[0].children.length!==1)odfFail('ambiguous document body');
    const presentation=bodies[0].children[0];if(presentation.namespaceURI!==ODF.office||presentation.localName!=='presentation')odfFail('expected a presentation body');
    const pages=odfChildren(presentation,ODF.draw,'page');if(pages.length>4096)odfFail('slide count exceeds bounds');
    const styles=archive.has('styles.xml')?parseXmlSource(decoder.decode(archive.read('styles.xml'))):null;
    if(styles&&(styles.root.namespaceURI!==ODF.office||styles.root.localName!=='document-styles'))odfFail('invalid styles root');
    return{archive,source,document,styles,pages};
}
export async function odpSlideIds(parsed){
    const result=[],ids=new Set(),aliases=new Set();
    for(const[ordinal,page]of parsed.pages.entries()){
        let identity=null;
        for(const[namespace,name,domain]of [[ODF.xml,'id','xml'],[ODF.draw,'id','draw'],[ODF.draw,'name','name']]){
            const value=odfAttribute(page,namespace,name);if(value===undefined)continue;
            if(!value||/[\u0000-\u001f\u007f-\u009f]/u.test(value))odfFail('invalid authored slide identity');
            const alias=JSON.stringify([domain,value]);if(aliases.has(alias))odfFail('ambiguous slide identity');aliases.add(alias);
            if(identity===null){
                const bytes=encoder.encode(value);
                identity=bytes.length<=128?`odp:${domain}:${value}`:`odp:sha256:${domain}:`+Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),v=>v.toString(16).padStart(2,'0')).join('');
            }
        }
        const anchored=odfAttribute(page,'urn:ricewm:deck:slide:1','id');
        if(anchored!==undefined&&(!anchored||odfBytes(anchored)>256||/[\u0000-\u001f\u007f]/u.test(anchored)))odfFail('invalid persistent slide identity');
        const id=anchored??identity??`odp:fallback:${ordinal+1}`;if(ids.has(id))odfFail('duplicate slide identity');ids.add(id);result.push(id);
    }
    return result;
}
export function odpObjects(page,slide){
    const result=new Map(),stack=[...page.children].reverse();let ordinal=0;
    while(stack.length){
        const node=stack.pop();if(node.namespaceURI!==ODF.draw)continue;
        if(node.localName==='g'){stack.push(...[...node.children].reverse());continue;}
        if(!['frame','rect','ellipse','line','custom-shape','path','polygon','polyline','connector'].includes(node.localName))continue;
        const anchor=odfAttribute(node,'urn:ricewm:deck:object:1','id');
        const id=anchor&&odfBytes(anchor)<=256?anchor:`${slide}/object:${ordinal}`;ordinal++;
        if(result.has(id))odfFail('ambiguous object identity');
        const bodies=node.localName==='frame'?odfChildren(node,ODF.draw,'text-box'):[node];
        result.set(id,{node,body:bodies.length===1?bodies[0]:null});
    }
    return result;
}
