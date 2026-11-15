import {parseXmlSource,escapeXmlAttribute} from '../xml-source.mjs';
import {checkedColorReference,resolveColorReference,colorWithOpacity,SCHEME_COLOR_SLOTS} from '../color-references.mjs';

const P='http://schemas.openxmlformats.org/presentationml/2006/main',A='http://schemas.openxmlformats.org/drawingml/2006/main';
const R='http://schemas.openxmlformats.org/officeDocument/2006/relationships',SR='http://purl.oclc.org/ooxml/officeDocument/relationships';
const SP='http://purl.oclc.org/ooxml/presentationml/main',SA='http://purl.oclc.org/ooxml/drawingml/main';
const decoder=new TextDecoder('utf-8',{fatal:true});
const attr=(n,k)=>n.attributes.find(a=>a.localName===k&&!a.namespaceURI)?.value;
const nodes=(n,ns,name)=>n?.children.filter(c=>c.namespaceURI===ns&&c.localName===name)||[];
const one=(n,ns,name)=>{const found=nodes(n,ns,name);if(found.length>1)throw TypeError('Ambiguous theme '+name);return found[0]??null;};
const bare=['comp','inv','gray'];
/** Parse only understood colour constructs. Unknown markup remains opaque. */
export function drawingmlColorReference(node,a) {
    if(!node||node.namespaceURI!==a)return null;
    const allowed={srgbClr:['val'],schemeClr:['val'],sysClr:['val','lastClr'],prstClr:['val'],scrgbClr:['r','g','b'],hslClr:['hue','sat','lum']},keys=allowed[node.localName];
    if(!keys||node.attributes.some(v=>v.namespaceURI!=='http://www.w3.org/2000/xmlns/'&&(v.namespaceURI||!keys.includes(v.localName))))return null;
    const transforms=[];
    for(const child of node.children){
        if(child.namespaceURI!==a||child.children.length||child.attributes.some(v=>v.namespaceURI!=='http://www.w3.org/2000/xmlns/'&&(v.namespaceURI||v.localName!=='val')))return null;
        const value=attr(child,'val');
        if(bare.includes(child.localName)){if(value!==undefined)return null;transforms.push({kind:child.localName});}
        else{if(!/^-?\d{1,10}$/u.test(value??''))return null;transforms.push({kind:child.localName,value:Number(value)});}
    }
    let source;
    if(node.localName==='srgbClr')source={kind:'srgb',value:'#'+(attr(node,'val')||''),transforms};
    else if(node.localName==='schemeClr')source={kind:'scheme',value:attr(node,'val'),transforms};
    else if(node.localName==='prstClr')source={kind:'preset',value:attr(node,'val'),transforms};
    else if(node.localName==='sysClr')source={kind:'system',name:attr(node,'val'),lastColor:'#'+(attr(node,'lastClr')||''),transforms};
    else{
        const values=(node.localName==='scrgbClr'?['r','g','b']:['hue','sat','lum']).map(k=>attr(node,k));
        if(values.some(v=>!/^-?\d{1,10}$/u.test(v??'')))return null;
        source={kind:node.localName==='scrgbClr'?'scrgb':'hsl',value:values.map(Number),transforms};
    }
    try{return checkedColorReference(source);}catch{return null;}
}
/** A bounded slide → layout → master → theme context, with closest overrides.
 * Relationships and content types come from the existing immutable OPC reader. */
export function readPptxSlideTheme(opc,slidePart,p) {
    if(![P,SP].includes(p))throw TypeError('Invalid presentation namespace');
    const a=p===P?A:SA,r=p===P?R:SR,alternate=p===P?SR:R,visited=new Set(),owners=[],overrides=[],sourceRoots=new WeakMap();
    const target=(owner,kind)=>{
        const found=opc.relationships(owner).filter(v=>v.type===r+'/'+kind||v.type===alternate+'/'+kind);
        if(found.length>1||found[0]&&(found[0].type!==r+'/'+kind||!found[0].part))throw TypeError('Ambiguous or external theme relationship');
        return found[0]?.part??null;
    };
    const load=(part,namespace,name,mime,track=true)=>{
        if(track){if(visited.has(part.toLowerCase()))throw TypeError('Cycle in theme inheritance');visited.add(part.toLowerCase());}
        if(mime&&opc.contentType(part)!==mime)throw TypeError('Invalid theme content type');
        const root=parseXmlSource(decoder.decode(opc.read(part))).root;
        if(root.namespaceURI!==namespace||root.localName!==name)throw TypeError('Invalid theme part root');sourceRoots.set(root,part);return root;
    };
    let owner=slidePart,root=load(owner,p,'sld'),theme=null;
    for(const [kind,name,mime]of [['slideLayout','sldLayout','application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml'],['slideMaster','sldMaster','application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml'],['theme','theme','application/vnd.openxmlformats-officedocument.theme+xml']]){
        owners.push({part:owner,root});const override=target(owner,'themeOverride');
        if(override)overrides.push(load(override,a,'themeOverride','application/vnd.openxmlformats-officedocument.themeOverride+xml',false));
        const next=target(owner,kind);if(!next)break;
        root=load(next,kind==='theme'?a:p,name,mime);owner=next;if(kind==='theme')theme=root;
    }
    const scheme=name=>overrides.map(v=>one(v,a,name)).find(Boolean)??one(one(theme,a,'themeElements'),a,name);
    const schemePart=name=>{const override=overrides.find(v=>one(v,a,name));return override?sourceRoots.get(override):theme?sourceRoots.get(theme):null;};
    const master=owners.find(v=>v.root.localName==='sldMaster')?.root;
    let map=one(master,p,'clrMap');
    for(const item of owners){
        if(item.root===master)break;
        const override=one(item.root,p,'clrMapOvr');
        if(override){const mapping=one(override,a,'overrideClrMapping'),masterMapping=one(override,a,'masterClrMapping');if(mapping&&masterMapping)throw TypeError('Ambiguous color map override');map=mapping??map;break;}
    }
    const defaults={bg1:'lt1',tx1:'dk1',bg2:'lt2',tx2:'dk2'},mapped=new Set(['bg1','tx1','bg2','tx2','accent1','accent2','accent3','accent4','accent5','accent6','hlink','folHlink']);
    const colors=scheme('clrScheme');
    function resolveScheme(slot,seen=new Set()){
        if(seen.has(slot)||seen.size>16)return null;seen=new Set(seen).add(slot);
        const name=mapped.has(slot)?attr(map||{attributes:[]},slot)??defaults[slot]??slot:slot;
        if(!SCHEME_COLOR_SLOTS.includes(name)||name==='phClr')return null;
        const entry=one(colors,a,name);if(!entry||entry.children.length!==1)return null;
        const source=drawingmlColorReference(entry.children[0],a);if(!source)return null;
        try{return resolveColorReference(source,next=>resolveScheme(next,seen));}catch{return null;}
    }
    const color=(node,placeholder=null)=>{
        let source=drawingmlColorReference(node,a);if(!source)return null;
        if(source.kind==='scheme'&&source.value==='phClr'){
            if(!placeholder)return null;source={...placeholder,transforms:[...placeholder.transforms,...source.transforms]};
        }
        try{return resolveColorReference(source,slot=>resolveScheme(slot));}catch{return null;}
    };
    const entries=SCHEME_COLOR_SLOTS.filter(slot=>slot!=='phClr').flatMap(slot=>{const value=resolveScheme(slot);return value?[{slot,color:value.color,opacity:value.opacity}]:[];});
    return {a,p,owners,themePart:theme?owner:null,themeRoot:theme,scheme,schemePart,color,resolveScheme,metadata:entries.length?{name:(attr(colors,'name')||attr(theme||{attributes:[]},'name')||'').slice(0,256),colors:entries}:null};
}
export function drawingmlColorXml(paint,a,context) {
    let source=paint.source?checkedColorReference(paint.source):{kind:'srgb',value:paint.color.toUpperCase(),transforms:[{kind:'alpha',value:Math.round(paint.opacity*100000)}]};
    if(paint.source){
        const actual=resolveColorReference(source,slot=>context.resolveScheme(slot));
        if(Math.abs(actual.opacity-paint.opacity)>1e-9)source=colorWithOpacity(actual,paint.opacity).source;
    }
    let name,attributes;
    if(source.kind==='system'){name='sysClr';attributes={val:source.name,lastClr:source.lastColor.slice(1)};}
    else if(source.kind==='hsl'){name='hslClr';attributes=Object.fromEntries(['hue','sat','lum'].map((k,i)=>[k,source.value[i]]));}
    else if(source.kind==='scrgb'){name='scrgbClr';attributes=Object.fromEntries(['r','g','b'].map((k,i)=>[k,source.value[i]]));}
    else{name={srgb:'srgbClr',scheme:'schemeClr',preset:'prstClr'}[source.kind];attributes={val:source.kind==='srgb'?source.value.slice(1):source.value};}
    const attributesXml=Object.entries(attributes).map(([k,v])=>` ${k}="${escapeXmlAttribute(String(v))}"`).join('');
    return `<rdcolor:${name} xmlns:rdcolor="${a}"${attributesXml}>${source.transforms.map(t=>`<rdcolor:${t.kind}${Object.hasOwn(t,'value')?' val="'+t.value+'"':''}/>`).join('')}</rdcolor:${name}>`;
}
