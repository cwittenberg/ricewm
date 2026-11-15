import {readPptxPackage} from './slide-package.mjs';
import {OpcPackage} from '../opc-package.mjs';
import {parseXmlSource,patchXmlSource,escapeXmlAttribute} from '../xml-source.mjs';
import {checkedBackgroundCommand} from './background-contract.mjs';
import {checkedBackgroundFill} from '../background-fills.mjs';
import {readPptxSlideTheme,drawingmlColorXml,drawingmlColorReference} from './drawingml-theme.mjs';
import {IMAGE_BYTES} from './image-contract.mjs';
import {checkedPictureFillSettings} from '../picture-fills.mjs';

export const PACKAGE_BACKGROUND_COMMANDS=new Set(['set-slide-background-style','set-slide-fill-background','set-slide-background-graphics','copy-slide-background']);
const P='http://schemas.openxmlformats.org/presentationml/2006/main',SP='http://purl.oclc.org/ooxml/presentationml/main';
const A='http://schemas.openxmlformats.org/drawingml/2006/main',SA='http://purl.oclc.org/ooxml/drawingml/main';
const R='http://schemas.openxmlformats.org/officeDocument/2006/relationships',SR='http://purl.oclc.org/ooxml/officeDocument/relationships';
const decoder=new TextDecoder('utf-8',{fatal:true}),encoder=new TextEncoder();
const attribute=(n,key)=>n.attributes.find(a=>a.localName===key&&!a.namespaceURI);
const attr=(n,key)=>attribute(n,key)?.value;
const children=(n,ns,name)=>n.children.filter(c=>c.namespaceURI===ns&&c.localName===name);
const fail=message=>{throw TypeError('Slide background: '+message);};
function source(parsed,opc,id,changes=new Map()) {
    const slide=parsed.result.slides.find(s=>s.id===id);if(!slide)fail('unknown slide');
    const p=parseXmlSource(parsed.result.xml).root.namespaceURI,r=p===P?R:SR;
    const rel=opc.relationships(parsed.path).find(item=>item.id===slide.relationship);
    if(!rel?.part||rel.type!==r+'/slide'||opc.contentType(rel.part)!=='application/vnd.openxmlformats-officedocument.presentationml.slide+xml')fail('invalid slide identity');
    const xml=changes.get(rel.part)??decoder.decode(opc.read(rel.part)),doc=parseXmlSource(xml),root=doc.root;
    if(![P,SP].includes(root.namespaceURI)||root.localName!=='sld')fail('invalid slide root');
    const common=children(root,root.namespaceURI,'cSld');if(common.length!==1)fail('ambiguous common slide');
    const backgrounds=children(common[0],root.namespaceURI,'bg');if(backgrounds.length>1)fail('ambiguous background');
    return {part:rel.part,xml,root,common:common[0],background:backgrounds[0]??null,p:root.namespaceURI,a:root.namespaceURI===P?A:SA};
}
function color(node,a,context,placeholder=null) {
    if(!node||node.children.length!==1)return null;
    const value=context.color(node.children[0],placeholder);if(!value)return null;
    // Keep the established literal-paint shape for unmodified RGB + alpha.
    if(value.source.kind==='srgb'&&value.source.transforms.length<=1&&value.source.transforms.every(t=>t.kind==='alpha')){
        const {source,...literal}=value;return literal;
    }
    return value;
}
function fillModel(fill,a,context,placeholder=null) {
    if(!fill)return null;
    if(fill.localName==='solidFill'){const value=color(fill,a,context,placeholder);return value?{kind:'solid',...value}:null;}
    if(fill.localName==='pattFill'){
        const fg=children(fill,a,'fgClr'),bg=children(fill,a,'bgClr');
        const foreground=fg.length===1?color(fg[0],a,context,placeholder):null,background=bg.length===1?color(bg[0],a,context,placeholder):null;
        if(!foreground||!background)return null;
        try{return checkedBackgroundFill({kind:'pattern',preset:attr(fill,'prst'),foreground,background});}catch{return null;}
    }
    if(fill.localName!=='gradFill'||attr(fill,'flip')&&!['none'].includes(attr(fill,'flip')))return null;
    const lists=children(fill,a,'gsLst'),lin=children(fill,a,'lin'),path=children(fill,a,'path'),tiles=children(fill,a,'tileRect');
    if(lists.length!==1||lin.length+path.length!==1||tiles.length>1||fill.children.some(n=>n.namespaceURI!==a||!['gsLst','lin','path','tileRect'].includes(n.localName)))return null;
    if(lists[0].children.some(n=>n.namespaceURI!==a||n.localName!=='gs'))return null;
    const stops=lists[0].children.map(n=>({offset:Number(attr(n,'pos'))/100000,...color(n,a,context,placeholder)}));
    const tile=tiles[0]?['l','t','r','b'].map(k=>Number(attr(tiles[0],k)??0)/100000):undefined;
    let gradient;
    if(lin.length){
        if(![undefined,'0','1','true','false'].includes(attr(lin[0],'scaled')))return null;
        gradient={kind:'linear',angle:Number(attr(lin[0],'ang')??0)/60000,scaled:['1','true'].includes(attr(lin[0],'scaled')),stops};
    }else{
        const kinds={circle:'radial',rect:'rectangular',shape:'path'},kind=kinds[attr(path[0],'path')],rects=children(path[0],a,'fillToRect');
        if(!kind||rects.length>1)return null;
        const focus=rects.length?['l','t','r','b'].map(k=>Number(attr(rects[0],k)??0)/100000):[0,0,0,0];
        gradient={kind,angle:0,focus,stops};
    }
    try{return checkedBackgroundFill({kind:'gradient',gradient:{...gradient,...(tile?{tile}: {})}});}catch{return null;}
}
function ownerFill(background,p,a,context) {
    const properties=children(background,p,'bgPr'),refs=children(background,p,'bgRef');
    if(properties.length===1&&!refs.length){
        const fills=properties[0].children.filter(n=>n.namespaceURI===a&&['solidFill','gradFill','pattFill','blipFill','noFill'].includes(n.localName));
        if(fills.length===1)return {kind:{solidFill:'solid',gradFill:'gradient',pattFill:'pattern',blipFill:'picture',noFill:'none'}[fills[0].localName],fill:fillModel(fills[0],a,context)};
    }else if(refs.length===1&&!properties.length){
        const index=attr(refs[0],'idx');
        if(!/^\d{1,4}$/u.test(index??'')||Number(index)<1001)return {kind:'unknown',fill:null};
        const styles=context.scheme('fmtScheme'),lists=styles?children(styles,a,'bgFillStyleLst'):[];
        const fill=lists.length===1?lists[0].children[Number(index)-1001]:null;
        const placeholder=refs[0].children.length===1?drawingmlColorReference(refs[0].children[0],a):null;
        if(fill?.namespaceURI===a&&placeholder){const value=fillModel(fill,a,context,placeholder);return {kind:fill.localName==='blipFill'?'picture':value?.kind??'unknown',fill:value};}
    }
    return {kind:'unknown',fill:null};
}
const STYLE_SLOTS=['bg1','bg2','tx2','tx1'];
function styleRecord(background,p,a) {
    if(!background)return null;
    const refs=children(background,p,'bgRef');if(refs.length!==1||children(background,p,'bgPr').length||refs[0].children.length!==1)return null;
    const color=drawingmlColorReference(refs[0].children[0],a),index=Number(attr(refs[0],'idx')),column=color?.kind==='scheme'&&!color.transforms.length?STYLE_SLOTS.indexOf(color.value):-1;
    return [1001,1002,1003].includes(index)&&column>=0?(index-1001)*4+column+1:null;
}
function styleFill(context,style,opc=null) {
    const formats=context.scheme('fmtScheme'),lists=formats?children(formats,context.a,'bgFillStyleLst'):[],fill=lists.length===1?lists[0].children[Math.floor((style-1)/4)]:null;
    if(fill?.namespaceURI!==context.a)return null;
    if(fill.localName==='blipFill'){try{return opc&&pictureFillRecord(opc,fill,context.schemePart('fmtScheme'),context.p,context.a)?{kind:'picture'}:null;}catch{return null;}}
    return fillModel(fill,context.a,context,{kind:'scheme',value:STYLE_SLOTS[(style-1)%4],transforms:[]});
}
/** The twelve presentation background references keep the source theme linked.
 * Preview swatches follow the compact Design gallery; actual fills come from
 * the theme's three bgFillStyleLst entries and current slide colour mapping. */
function styleCatalog(context,opc) {
    return Array.from({length:12},(_,index)=>{
        const base=context.resolveScheme(['lt1','lt2','dk2','dk1'][index%4]);let preview=null;
        if(base){
            const amount=index<4?0:index<8?.15:.12,target=index<8?128:0;
            const channels=[1,3,5].map(at=>parseInt(base.color.slice(at,at+2),16));
            preview={color:'#'+channels.map(value=>Math.round(value+(target-value)*amount).toString(16).padStart(2,'0')).join('').toUpperCase(),opacity:base.opacity+(1-base.opacity)*amount};
        }
        return {style:index+1,label:`Style ${index+1}`,preview,available:Boolean(styleFill(context,index+1,opc))};
    });
}
function metadata(s,opc) {
    const context=readPptxSlideTheme(opc,s.part,s.p),local=s.background?ownerFill(s.background,s.p,s.a,context):{kind:'layout',fill:null};
    let effectiveFill=local.fill;
    if(!s.background)for(const {root}of context.owners.slice(1)){
        const common=children(root,s.p,'cSld');if(common.length!==1)throw TypeError('Ambiguous inherited common slide');
        const backgrounds=children(common[0],s.p,'bg');if(backgrounds.length>1)throw TypeError('Ambiguous inherited background');
        if(backgrounds.length){effectiveFill=ownerFill(backgrounds[0],s.p,s.a,context).fill;break;}
    }
    return {...local,effectiveFill,theme:context.metadata,style:styleRecord(s.background,s.p,s.a),styles:styleCatalog(context,opc),hidden:['0','false'].includes(attr(s.root,'showMasterSp'))};
}
/** Local source identity and full editable paint. Unknown source constructs
 * are preserved; a caller can still explicitly choose a replacement fill. */
export function inspectPptxBackground(input,slide) {
    const parsed=readPptxPackage(input),opc=new OpcPackage(parsed.archive),s=source(parsed,opc,slide);
    return metadata(s,opc);
}
const knownAttributes=(node,keys)=>node.attributes.every(a=>a.namespaceURI==='http://www.w3.org/2000/xmlns/'||!a.namespaceURI&&keys.includes(a.localName));
const integer=(node,key,fallback)=>{const value=attr(node,key);if(value===undefined)return fallback;if(!/^-?\d{1,12}$/u.test(value))throw TypeError('Invalid picture fill coordinate');return Number(value);};
function pictureFillRecord(opc,fill,ownerPart,p,a){
    if(!ownerPart)return null;
        if(!knownAttributes(fill,['dpi','rotWithShape'])||fill.children.some(n=>n.namespaceURI!==a||!['blip','tile','stretch','srcRect'].includes(n.localName)))return null;
        const blips=children(fill,a,'blip'),tiles=children(fill,a,'tile'),stretches=children(fill,a,'stretch'),crops=children(fill,a,'srcRect');
        if(blips.length!==1||tiles.length+stretches.length!==1||crops.length>1)return null;
        const blip=blips[0],embed=blip.attributes.find(a=>a.localName==='embed'&&a.namespaceURI===(p===P?R:SR));
        if(!embed||blip.attributes.some(a=>a!==embed&&a.namespaceURI!=='http://www.w3.org/2000/xmlns/'&&!(a.namespaceURI===''&&a.localName==='cstate')))return null;
        const rel=opc.relationships(ownerPart).find(r=>r.id===embed.value);
        if(!rel?.part||rel.type!==(p===P?R:SR)+'/image')return null;
        const mime=opc.contentType(rel.part);if(!['image/png','image/jpeg','image/svg+xml'].includes(mime))return null;
        let opacity=1;
        for(const effect of blip.children){if(effect.namespaceURI!==a||effect.localName!=='alphaModFix'||effect.children.length||!knownAttributes(effect,['amt']))return null;opacity*=integer(effect,'amt',100000)/100000;}
        if(crops.length&&(!knownAttributes(crops[0],['l','t','r','b'])||crops[0].children.length||['l','t','r','b'].some(k=>integer(crops[0],k,0)!==0)))return null;
        let settings;
        if(tiles.length){const tile=tiles[0];if(!knownAttributes(tile,['tx','ty','sx','sy','flip','algn'])||tile.children.length)return null;
            settings={mode:'tile',opacity,scaleX:integer(tile,'sx',100000)/100000,scaleY:integer(tile,'sy',100000)/100000,offsetX:integer(tile,'tx',0)/9525,offsetY:integer(tile,'ty',0)/9525,alignment:attr(tile,'algn')??'tl',mirror:attr(tile,'flip')??'none'};
        }else{const stretch=stretches[0],rects=children(stretch,a,'fillRect');
            if(!knownAttributes(stretch,[])||stretch.children.length!==rects.length||rects.length>1||rects.some(rect=>!knownAttributes(rect,['l','t','r','b'])||rect.children.length||['l','t','r','b'].some(k=>integer(rect,k,0)!==0)))return null;
            settings={mode:'stretch',opacity,scaleX:1,scaleY:1,offsetX:0,offsetY:0,alignment:'tl',mirror:'none'};
        }
        settings=checkedPictureFillSettings(settings);
        const dpi=integer(fill,'dpi',0);if(dpi<0||dpi>0xffffffff)return null;
        const bytes=opc.read(rel.part);if(bytes.length>IMAGE_BYTES||!bytes.length)return null;
        return {part:rel.part,owner:ownerPart,mime,bytes,settings,dpi};
}
/** Understood embedded picture source only. Validation of the actual image is
 * still performed by the retained native decoder before a scene is enriched. */
export function inspectPptxPictureBackground(input,slide) {
    const parsed=readPptxPackage(input),opc=new OpcPackage(parsed.archive),s=source(parsed,opc,slide),context=readPptxSlideTheme(opc,s.part,s.p);
    for(const owner of context.owners){
        const common=children(owner.root,s.p,'cSld');if(common.length!==1)return null;
        const backgrounds=children(common[0],s.p,'bg');if(backgrounds.length>1)return null;if(!backgrounds.length)continue;
        const props=children(backgrounds[0],s.p,'bgPr'),refs=children(backgrounds[0],s.p,'bgRef');
        if(refs.length===1&&!props.length){
            const raw=attr(refs[0],'idx');if(!/^\d{1,4}$/u.test(raw??'')||Number(raw)<1001||refs[0].children.length!==1||!drawingmlColorReference(refs[0].children[0],s.a))return null;
            const matrix=context.scheme('fmtScheme'),lists=matrix?children(matrix,s.a,'bgFillStyleLst'):[],fill=lists.length===1?lists[0].children[Number(raw)-1001]:null;
            return fill?.namespaceURI===s.a&&fill.localName==='blipFill'?pictureFillRecord(opc,fill,context.schemePart('fmtScheme'),s.p,s.a):null;
        }
        if(props.length!==1||refs.length)return null;
        const fills=props[0].children.filter(n=>n.namespaceURI===s.a&&['solidFill','gradFill','pattFill','blipFill','noFill'].includes(n.localName));
        if(fills.length!==1||fills[0].localName!=='blipFill')return null;
        return pictureFillRecord(opc,fills[0],owner.part,s.p,s.a);
    }
    return null;
}
/** Called only for bytes already accepted by the native image decoder. Match
 * its PNG pHYs / JPEG JFIF physical density, including integer ppm rounding. */
export function pictureSourceSettings(record) {
    const settings={...record.settings};if(settings.mode!=='tile')return settings;
    const bytes=record.bytes,view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);let ppm=null;
    if(!record.dpi&&record.mime==='image/png')for(let at=8;at+12<=bytes.length;){
        const length=view.getUint32(at),end=at+12+length;if(end>bytes.length)throw TypeError('Invalid validated PNG density');
        if(length===9&&String.fromCharCode(...bytes.subarray(at+4,at+8))==='pHYs'&&bytes[at+16]===1){const x=view.getUint32(at+8),y=view.getUint32(at+12);if(x&&y)ppm=[x,y];}
        at=end;
    }
    if(!record.dpi&&record.mime==='image/jpeg')for(let at=2;at+4<=bytes.length;){
        if(bytes[at]!==255)throw TypeError('Invalid validated JPEG density');const marker=bytes[at+1];if(marker===218||marker===217)break;if(marker===255){at++;continue;}
        const length=view.getUint16(at+2),end=at+2+length;if(length<2||end>bytes.length)throw TypeError('Invalid validated JPEG segment');
        const start=at+4;
        if(marker===224&&length>=14&&String.fromCharCode(...bytes.subarray(start,start+5))==='JFIF\0'){
            const units=bytes[start+7],x=view.getUint16(start+8),y=view.getUint16(start+10),convert=n=>units===1?Math.floor((n*10000+127)/254):n*100;
            if([1,2].includes(units)&&x&&y)ppm=[convert(x),convert(y)];
        }
        at=end;
    }
    const dpi=record.dpi?[record.dpi,record.dpi]:ppm?ppm.map(n=>n*.0254):[96,96];
    settings.scaleX*=96/dpi[0];settings.scaleY*=96/dpi[1];return checkedPictureFillSettings(settings);
}
/** Add tile options to the genuine native-written picture background. Image
 * relationships, image bytes, alpha, scaling and offsets remain native-owned. */
export function editPptxPictureOptions(input,command) {
    const c=checkedBackgroundCommand(command);if(c.type!=='set-slide-picture-background')fail('expected picture background');
    const parsed=readPptxPackage(input),opc=new OpcPackage(parsed.archive),changes=new Map();
    for(const id of c.slide===null?parsed.result.slides.map(s=>s.id):[c.slide]){
        const s=source(parsed,opc,id),props=s.background?children(s.background,s.p,'bgPr'):[],fills=props.length===1?children(props[0],s.a,'blipFill'):[],tiles=fills.length===1?children(fills[0],s.a,'tile'):[];
        if(c.mode!=='tile')continue;if(tiles.length!==1)fail('native-written tile is missing');
        const tile=tiles[0],edits=[];
        for(const [key,value]of [['algn',c.alignment??'tl'],['flip',c.mirror??'none']]){
            const old=attribute(tile,key);edits.push(old?{start:old.valueStart,end:old.valueEnd,text:value}:{start:tile.openEnd-(tile.selfClosing?2:1),end:tile.openEnd-(tile.selfClosing?2:1),text:` ${key}="${value}"`});
        }
        changes.set(s.part,encoder.encode(patchXmlSource(s.xml,edits)));
    }
    return {bytes:changes.size?parsed.archive.replace(changes):input.slice(),changedParts:[...changes.keys()]};
}
function colorXml(paint,context) {return drawingmlColorXml(paint,context.a,context);}
function fillXml(fill,context) {
    if(fill.kind==='solid')return `<rdfill:solidFill>${colorXml(fill,context)}</rdfill:solidFill>`;
    if(fill.kind==='pattern')return `<rdfill:pattFill prst="${fill.preset}"><rdfill:fgClr>${colorXml(fill.foreground,context)}</rdfill:fgClr><rdfill:bgClr>${colorXml(fill.background,context)}</rdfill:bgClr></rdfill:pattFill>`;
    const g=fill.gradient,rect=(name,values)=>`<rdfill:${name} ${['l','t','r','b'].map((k,i)=>`${k}="${Math.round(values[i]*100000)}"`).join(' ')}/>`;
    const stops=g.stops.map(s=>`<rdfill:gs pos="${Math.round(s.offset*100000)}">${colorXml(s,context)}</rdfill:gs>`).join('');
    const geometry=g.kind==='linear'?`<rdfill:lin ang="${Math.round(((g.angle%360+360)%360)*60000)}" scaled="${g.scaled?'1':'0'}"/>`
        :`<rdfill:path path="${{radial:'circle',rectangular:'rect',path:'shape'}[g.kind]}">${rect('fillToRect',g.focus||[.5,.5,.5,.5])}</rdfill:path>`;
    return `<rdfill:gradFill rotWithShape="0"><rdfill:gsLst>${stops}</rdfill:gsLst>${geometry}${g.tile?rect('tileRect',g.tile):''}</rdfill:gradFill>`;
}
function replaceBackground(s,xml) {
    if(s.background)return patchXmlSource(s.xml,[{start:s.background.start,end:s.background.end,text:xml}]);
    if(!xml)return s.xml;
    if(s.common.selfClosing)fail('common slide has no shape tree');
    return patchXmlSource(s.xml,[{start:s.common.openEnd,end:s.common.openEnd,text:xml}]);
}
function copyBackground(s) {
    if(!s.background)return '';
    // Picture resource relationships are owned by each slide and must transfer
    // through the existing native resource channel, never by copying rIds.
    const doc=parseXmlSource(s.xml);
    if(doc.nodes.some(n=>n.start>=s.background.start&&n.end<=s.background.end&&n.attributes.some(a=>[R,SR].includes(a.namespaceURI))))fail('copy picture resources through the picture background command');
    const own=new Set(s.background.attributes.filter(a=>a.name==='xmlns'||a.prefix==='xmlns').map(a=>a.name==='xmlns'?'':a.localName));
    const declarations=Object.entries(s.background.namespaces).filter(([key])=>key!=='xml'&&!own.has(key)).map(([key,value])=>` xmlns${key?':'+key:''}="${escapeXmlAttribute(value)}"`).join('');
    const offset=s.background.openEnd-(s.background.selfClosing?2:1);
    return s.xml.slice(s.background.start,offset)+declarations+s.xml.slice(offset,s.background.end);
}
/** Produce a source-preserving candidate. Native source guards and successful
 * reimport are required in the owning transaction before acknowledging edits. */
export function editPptxBackground(input,commands) {
    if(!Array.isArray(commands)||!commands.length||commands.length>32)fail('expected 1–32 commands');
    commands=commands.map(c=>{const value=checkedBackgroundCommand(c);if(!PACKAGE_BACKGROUND_COMMANDS.has(value.type))fail('unsupported command');return value;});
    const parsed=readPptxPackage(input),opc=new OpcPackage(parsed.archive),changes=new Map();
    for(const c of commands){
        const copy=c.type==='copy-slide-background'?copyBackground(source(parsed,opc,c.source,changes)):null;
        const targets=c.slide===null?parsed.result.slides.map(s=>s.id):[c.slide];
        for(const id of targets){
            const s=source(parsed,opc,id,changes);let xml;
            if(c.type==='set-slide-background-graphics'){
                const old=attribute(s.root,'showMasterSp'),value=c.hidden?'0':'1';
                xml=patchXmlSource(s.xml,[old?{start:old.valueStart,end:old.valueEnd,text:value}:{start:s.root.openEnd-1,end:s.root.openEnd-1,text:` showMasterSp="${value}"`}]);
            }else if(c.type==='set-slide-background-style'){
                const context=readPptxSlideTheme(opc,s.part,s.p);
                if(!styleFill(context,c.style,opc))fail('background style is not supported by this slide theme');
                const index=1001+Math.floor((c.style-1)/4),slot=STYLE_SLOTS[(c.style-1)%4];
                xml=replaceBackground(s,`<rdbg:bg xmlns:rdbg="${s.p}" xmlns:rdstyle="${s.a}"><rdbg:bgRef idx="${index}"><rdstyle:schemeClr val="${slot}"/></rdbg:bgRef></rdbg:bg>`);
            }else if(c.type==='copy-slide-background')xml=replaceBackground(s,copy);
            else{
                const paint=fillXml(c.fill,readPptxSlideTheme(opc,s.part,s.p)),properties=s.background?children(s.background,s.p,'bgPr'):[];
                const paints=properties.length===1?properties[0].children.filter(n=>n.namespaceURI===s.a&&['solidFill','gradFill','pattFill','blipFill','noFill'].includes(n.localName)):[];
                if(paints.length===1){
                    const n=paints[0],value=paint.replace(/^<rdfill:([A-Za-z]+)/u,`<rdfill:$1 xmlns:rdfill="${s.a}"`);
                    xml=patchXmlSource(s.xml,[{start:n.start,end:n.end,text:value}]);
                }else xml=replaceBackground(s,`<rdbg:bg xmlns:rdbg="${s.p}" xmlns:rdfill="${s.a}"><rdbg:bgPr>${paint}<rdfill:effectLst/></rdbg:bgPr></rdbg:bg>`);
            }
            changes.set(s.part,xml);
        }
    }
    return {bytes:parsed.archive.replace(new Map([...changes].map(([part,xml])=>[part,encoder.encode(xml)]))),changedParts:[...changes.keys()]};
}
