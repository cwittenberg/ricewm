import {parseXmlSource,patchXmlSource,escapeXmlAttribute} from '../xml-source.mjs';
import {ODF,odfAttribute,odfChildren,odfFragment,odfAppend,odfSetAttributes,odfFail,odfRaw} from './odp-source.mjs';

const attr=odfAttribute,children=odfChildren,escape=escapeXmlAttribute;
const expanded=a=>JSON.stringify([a.namespaceURI,a.localName]);
const declarations=`xmlns:rrst="${ODF.style}" xmlns:rrfo="${ODF.fo}" xmlns:rrsvg="${ODF.svg}"`;
const canonical=values=>JSON.stringify([...values].sort(([a],[b])=>a.localeCompare(b)));
export class OdpTextStyles{
    constructor(parsed){
        this.parsed=parsed;this.documents=[parsed.document,...(parsed.styles?[parsed.styles]:[])];this.names=new Set();this.created=[];this.fonts=[];this.cache=new Map();this.fontCache=new Map();this.serial=0;
        for(const doc of this.documents)for(const node of doc.nodes){const name=attr(node,ODF.style,'name');if(name!==undefined)this.names.add(name);}
        // Reuse equivalent direct styles instead of growing one style per key.
        for(const node of children(parsed.document.root,ODF.office,'automatic-styles').flatMap(n=>n.children)){
            if(node.namespaceURI!==ODF.style||node.localName!=='style'||attr(node,ODF.style,'family')!=='text'||node.attributes.some(a=>a.namespaceURI!==ODF.xmlns&&!(a.namespaceURI===ODF.style&&['name','family'].includes(a.localName))))continue;
            const props=children(node,ODF.style,'text-properties');if(props.length!==1||node.children.length!==1||props[0].children.length)continue;
            this.cache.set(canonical(props[0].attributes.filter(a=>a.namespaceURI!==ODF.xmlns).map(a=>[expanded(a),a.value])),attr(node,ODF.style,'name'));
        }
    }
    name(){let name;do{name='RiceTextRange'+(++this.serial);}while(this.names.has(name));this.names.add(name);return name;}
    retain(value){if(this.created.reduce((n,s)=>n+s.length,0)+this.fonts.reduce((n,s)=>n+s.length,0)+value.length>8*1024*1024)odfFail('style source exceeds bounds');return value;}
    direct(values,childXml=''){
        const map=new Map(values.map(([namespace,name,value])=>[JSON.stringify([namespace,name]),String(value)])),key=childXml?'':canonical(map);
        if(key&&this.cache.has(key))return this.cache.get(key);
        const name=this.name(),prefixes=new Map([[ODF.style,'rrst'],[ODF.fo,'rrfo'],[ODF.svg,'rrsvg']]);let extra='',properties='';
        for(const[key,value]of map){const[namespace,local]=JSON.parse(key);let prefix='';if(namespace){prefix=prefixes.get(namespace);if(!prefix){prefix='rrp'+prefixes.size;prefixes.set(namespace,prefix);extra+=` xmlns:${prefix}="${escape(namespace)}"`;}}properties+=` ${prefix?prefix+':':''}${local}="${escape(value)}"`;}
        const xml=`<rrst:style ${declarations}${extra} rrst:name="${name}" rrst:family="text"><rrst:text-properties${properties}>${childXml}</rrst:text-properties></rrst:style>`;
        this.created.push(this.retain(xml));if(key)this.cache.set(key,name);return name;
    }
    font(family){
        if(this.fontCache.has(family))return this.fontCache.get(family);
        for(const doc of this.documents)for(const face of children(doc.root,ODF.office,'font-face-decls').flatMap(n=>children(n,ODF.style,'font-face'))){
            if(attr(face,ODF.svg,'font-family')?.replace(/^['"]|['"]$/gu,'')===family&&doc===this.parsed.document){const name=attr(face,ODF.style,'name');if(name){this.fontCache.set(family,name);return name;}}
        }
        const name=this.name();this.fonts.push(this.retain(`<rrst:font-face ${declarations} rrst:name="${name}" rrsvg:font-family="${escape(family)}"/>`));this.fontCache.set(family,name);return name;
    }
    relocateFont(owner,name){
        const faces=doc=>children(doc.root,ODF.office,'font-face-decls').flatMap(n=>children(n,ODF.style,'font-face')).filter(n=>attr(n,ODF.style,'name')===name);
        const local=faces(owner),found=local.length?local.map(node=>({node,doc:owner})):this.documents.filter(doc=>doc!==owner&&doc.root.localName==='document-styles').flatMap(doc=>faces(doc).map(node=>({node,doc})));
        if(found.length!==1)odfFail('unresolved or ambiguous font face');
        const {node,doc}=found[0];if(doc===this.parsed.document)return name;
        const key=JSON.stringify(['relocated-font',this.documents.indexOf(doc),name]);if(this.fontCache.has(key))return this.fontCache.get(key);
        const alias=this.name(),xml=odfFragment(doc.source,node),parsed=parseXmlSource(xml);
        this.fonts.push(this.retain(odfSetAttributes(xml,parsed.root,[[ODF.style,'name',alias]])));this.fontCache.set(key,alias);return alias;
    }
    characters(style){
        const values=[];
        for(const[key,value]of Object.entries(style)){
            if(key==='color')values.push([ODF.fo,'color',value]);
            if(key==='underline')values.push([ODF.style,'text-underline-style',value?'solid':'none'],[ODF.style,'text-underline-type',value?'single':'none']);
            if(key==='fontFamily'){
                const font=this.font(value);values.push([ODF.fo,'font-family',value],[ODF.style,'font-name',font]);
                for(const suffix of ['asian','complex'])values.push([ODF.style,'font-family-'+suffix,value],[ODF.style,'font-name-'+suffix,font]);
            }
            const property={bold:'font-weight',italic:'font-style',fontSize:'font-size'}[key];
            if(property){const rendered=key==='bold'?(value?'bold':'normal'):key==='italic'?(value?'italic':'normal'):(value*.75)+'pt';values.push([ODF.fo,property,rendered]);for(const suffix of ['asian','complex'])values.push([ODF.style,property+'-'+suffix,rendered]);}
        }
        return this.direct(values);
    }
    wrap(name,content,context=null){
        if(!content)return '';
        if(context?.namespaceURI===ODF.text&&context.localName==='span'&&attr(context,ODF.text,'style-name')===name)return content;
        return `<rrt:span xmlns:rrt="${ODF.text}" rrt:style-name="${escape(name)}">${content}</rrt:span>`;
    }
    resolve(name,family,{commonOnly=false,owner=this.parsed.document}={}){
        const common=[],local=[],external=[];
        for(const doc of this.documents)for(const group of doc.root.children){
            if(group.namespaceURI!==ODF.office||!(commonOnly?group.localName==='styles':['automatic-styles','styles'].includes(group.localName)))continue;
            for(const node of children(group,ODF.style,'style'))if(attr(node,ODF.style,'name')===name&&attr(node,ODF.style,'family')===family){const record={node,doc,common:group.localName==='styles'};(record.common?common:doc===owner?local:doc.root.localName==='document-styles'?external:[]).push(record);}
        }
        const found=[...common,...(commonOnly?[]:local.length?local:external)];
        if(found.length!==1)odfFail('unresolved or ambiguous '+family+' style');return found[0];
    }
    chain(name,family){
        const result=[],seen=new Set();let commonOnly=false;
        while(name!==undefined){
            if(seen.has(name)||seen.size>=64)odfFail('cyclic or excessive style inheritance');seen.add(name);
            const value=this.resolve(name,family,{commonOnly});if(children(value.node,ODF.style,'map').length)odfFail('conditional styles retain their native edit guard');result.push(value);
            name=attr(value.node,ODF.style,'parent-style-name');commonOnly=true;
        }
        return result;
    }
    paragraph(row,align){
        const original=attr(row.node,ODF.text,'style-name'),key=JSON.stringify(['paragraph',original??null,align]);if(this.cache.has(key))return this.cache.get(key);
        const name=this.name();let xml;
        if(original!==undefined){
            const record=this.resolve(original,'paragraph');
            if(record.common)xml=`<rrst:style ${declarations} rrst:name="${name}" rrst:family="paragraph" rrst:parent-style-name="${escape(original)}"/>`;
            else{xml=odfFragment(record.doc.source,record.node);let d=parseXmlSource(xml);xml=odfSetAttributes(xml,d.root,[[ODF.style,'name',name]]);}
        }else xml=`<rrst:style ${declarations} rrst:name="${name}" rrst:family="paragraph"/>`;
        let doc=parseXmlSource(xml),props=children(doc.root,ODF.style,'paragraph-properties');if(props.length>1)odfFail('ambiguous paragraph properties');
        xml=props.length?odfSetAttributes(xml,props[0],[[ODF.fo,'text-align',align]]):odfAppend(xml,doc.root,`<rrst:paragraph-properties xmlns:rrst="${ODF.style}" xmlns:rrfo="${ODF.fo}" rrfo:text-align="${align}"/>`);
        this.created.push(this.retain(xml));this.cache.set(key,name);return name;
    }
    paragraphText(row){
        const values=new Map(),fragments=new Map(),name=attr(row.node,ODF.text,'style-name');
        const records=name===undefined?[]:this.chain(name,'paragraph');
        let shape=row.node.parent;while(shape&&!(shape.namespaceURI===ODF.table&&shape.localName==='table-cell')&&(shape.namespaceURI!==ODF.draw||shape.localName==='text-box'))shape=shape.parent;
        if(shape?.namespaceURI===ODF.table){
            const name=attr(shape,ODF.table,'style-name')??attr(shape.parent,ODF.table,'default-cell-style-name');
            if(name!==undefined)records.push(...this.chain(name,'table-cell'));
            const defaults=this.documents.flatMap(doc=>children(doc.root,ODF.office,'styles').flatMap(group=>children(group,ODF.style,'default-style').filter(n=>attr(n,ODF.style,'family')==='table-cell').map(node=>({node,doc}))));
            if(defaults.length>1)odfFail('ambiguous table cell defaults');records.push(...defaults);
        }else if(shape)for(const[namespace,family]of [[ODF.draw,'graphic'],['urn:oasis:names:tc:opendocument:xmlns:presentation:1.0','presentation']]){
            const name=attr(shape,namespace,'style-name');if(name!==undefined)records.push(...this.chain(name,family));
        }
        const defaults=this.documents.flatMap(doc=>children(doc.root,ODF.office,'styles').flatMap(group=>children(group,ODF.style,'default-style').filter(n=>attr(n,ODF.style,'family')==='paragraph').map(node=>({node,doc}))));
        if(defaults.length>1)odfFail('ambiguous paragraph defaults');records.push(...defaults);
        for(const{node,doc}of records){
            const properties=children(node,ODF.style,'text-properties');if(properties.length>1)odfFail('ambiguous paragraph text properties');
            for(const p of properties){
                const familyKey=JSON.stringify([ODF.fo,'font-family']);
                if(!values.has(familyKey)){
                    const face=attr(p,ODF.style,'font-name'),family=attr(p,ODF.fo,'font-family');
                    if(face!==undefined){
                        const alias=this.relocateFont(doc,face);values.set(JSON.stringify([ODF.style,'font-name']),[ODF.style,'font-name',alias]);
                        // A resolved font reference also owns precedence over
                        // a more distant ancestor's fo:font-family.
                        values.set(familyKey,null);
                    }else if(family!==undefined)values.set(familyKey,[ODF.fo,'font-family',family]);
                }
                for(const a of p.attributes)if(a.namespaceURI!==ODF.xmlns&&!values.has(expanded(a))){
                    if(a.namespaceURI===ODF.style&&a.localName==='font-name'||a.namespaceURI===ODF.fo&&a.localName==='font-family')continue;
                    const value=a.namespaceURI===ODF.style&&['font-name','font-name-asian','font-name-complex'].includes(a.localName)?this.relocateFont(doc,a.value):a.value;
                    values.set(expanded(a),[a.namespaceURI,a.localName,value]);
                }
                for(const child of p.children){const key=JSON.stringify([child.namespaceURI,child.localName]);if(!fragments.has(key))fragments.set(key,odfFragment(doc.source,child));}
            }
        }
        return{values,fragments};
    }
    tailInheritance(head,tail){
        if(attr(head.node,ODF.text,'style-name')===attr(tail.node,ODF.text,'style-name'))return null;
        const before=this.paragraphText(head),after=this.paragraphText(tail);
        for(const[property,value]of [['font-weight','normal'],['font-style','normal']]){
            const key=JSON.stringify([ODF.fo,property]);if(before.values.has(key)&&!after.values.has(key))after.values.set(key,[ODF.fo,property,value]);
        }
        if(before.values.has(JSON.stringify([ODF.style,'text-underline-style']))&&!after.values.has(JSON.stringify([ODF.style,'text-underline-style'])))after.values.set(JSON.stringify([ODF.style,'text-underline-style']),[ODF.style,'text-underline-style','none']);
        for(const property of ['font-family','font-size','color']){
            const key=JSON.stringify([ODF.fo,property]);if(before.values.has(key)&&!after.values.has(key))odfFail('paragraph join needs an explicit inherited '+property);
        }
        return this.direct([...after.values.values()].filter(Boolean),[...after.fragments.values()].join(''));
    }
    patches(){
        const patches=[],insertions=new Map(),root=this.parsed.document.root;
        for(const[kind,xml]of [['font-face-decls',this.fonts.join('')],['automatic-styles',this.created.join('')]]){
            if(!xml)continue;const groups=children(root,ODF.office,kind);if(groups.length>1)odfFail('ambiguous '+kind);
            if(groups.length){const n=groups[0];patches.push({start:n.start,end:n.end,text:odfAppend(odfRaw(this.parsed.source,n),{...n,start:0,openEnd:n.openEnd-n.start,closeStart:n.closeStart===null?null:n.closeStart-n.start,end:n.end-n.start},xml)});}
            else{const at=kind==='font-face-decls'?(children(root,ODF.office,'automatic-styles')[0]?.start??children(root,ODF.office,'body')[0].start):children(root,ODF.office,'body')[0].start;insertions.set(at,(insertions.get(at)||'')+`<rro:${kind} xmlns:rro="${ODF.office}">${xml}</rro:${kind}>`);}
        }
        for(const[start,text]of insertions){const replacement=patches.find(p=>p.start===start);if(replacement)replacement.text=text+replacement.text;else patches.push({start,end:start,text});}return patches;
    }
}
