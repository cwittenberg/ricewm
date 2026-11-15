import {readPptxPackage} from './slide-package.mjs';
import {OpcPackage} from '../opc-package.mjs';
import {parseXmlSource,patchXmlSource,escapeXmlAttribute} from '../xml-source.mjs';
import {textRangeIndex} from '../text-ranges.mjs';
import {checkedTextRangeCommand,textRangeTargetKey} from './text-range-contract.mjs';
import {cellKey} from './table-values.mjs';
import {textParagraphsInRange,validBulletGlyph,validParagraphMarker} from './paragraph-format.mjs';
import {readPptxSlideTheme} from './drawingml-theme.mjs';

const P='http://schemas.openxmlformats.org/presentationml/2006/main',SP='http://purl.oclc.org/ooxml/presentationml/main';
const A='http://schemas.openxmlformats.org/drawingml/2006/main',SA='http://purl.oclc.org/ooxml/drawingml/main';
const R='http://schemas.openxmlformats.org/officeDocument/2006/relationships',SR='http://purl.oclc.org/ooxml/officeDocument/relationships';
const XML='http://www.w3.org/XML/1998/namespace',XMLNS='http://www.w3.org/2000/xmlns/';
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const bulletChars={disc:'•',circle:'○',square:'▪'};
const numberSchemes={decimal:'arabicPeriod','lower-alpha':'alphaLcPeriod','upper-alpha':'alphaUcPeriod','lower-roman':'romanLcPeriod','upper-roman':'romanUcPeriod','decimal-parenthesis':'arabicParenR','lower-alpha-parenthesis':'alphaLcParenR'};
const fail=message=>{throw new TypeError('Presentation text range: '+message);};
const attr=(node,name,namespace='')=>node.attributes.find(a=>a.localName===name&&a.namespaceURI===namespace)?.value;
const children=(node,ns,name)=>node.children.filter(n=>n.namespaceURI===ns&&n.localName===name);
const raw=(source,node)=>source.slice(node.start,node.end);
const escaped=value=>escapeXmlAttribute(value).replaceAll('&quot;','"').replaceAll('&apos;',"'");
function leafText(source,node) {
    if(node.children.length)fail('nested text leaf');
    const text=node.selfClosing?'':source.slice(node.openEnd,node.closeStart);
    let result='',offset=0;
    const markup=/<!--[\s\S]*?-->|<!\[CDATA\[([\s\S]*?)\]\]>|<\?[\s\S]*?\?>/gu;
    const decode=value=>value.replace(/&([^;]+);/gu,(_,name)=>{
        const named={amp:'&',lt:'<',gt:'>',quot:'"',apos:"'"};
        return Object.hasOwn(named,name)?named[name]:String.fromCodePoint(name.startsWith('#x')?parseInt(name.slice(2),16):Number(name.slice(1)));
    });
    for(const match of text.matchAll(markup)) {result+=decode(text.slice(offset,match.index));if(match[1]!==undefined)result+=match[1];offset=match.index+match[0].length;}
    result+=decode(text.slice(offset));
    return result.replaceAll('\r\n','\n').replaceAll('\r','\n');
}
function replaceLeaf(source,node,text) {
    let opening=source.slice(node.start,node.openEnd),space=node.attributes.find(a=>a.namespaceURI===XML&&a.localName==='space');
    opening=space?patchXmlSource(opening,[{start:space.valueStart-node.start,end:space.valueEnd-node.start,text:'preserve'}]):opening;
    const end=opening.length-(node.selfClosing?2:1);
    opening=opening.slice(0,end)+(space?'':' xml:space="preserve"')+'>';
    return opening+escaped(text)+`</${node.name}>`;
}
function namespaceDeclarations(node) {
    const declared=new Set(node.attributes.filter(a=>a.namespaceURI===XMLNS).map(a=>a.name==='xmlns'?'':a.localName));
    return Object.entries(node.namespaces).filter(([prefix])=>prefix!=='xml'&&!declared.has(prefix))
        .map(([prefix,uri])=>` xmlns${prefix?':'+prefix:''}="${escapeXmlAttribute(uri)}"`).join('');
}
function movedFragment(source,node,fragment) {
    const openEnd=fragment.indexOf('>'),at=fragment[openEnd-1]==='/'?openEnd-1:openEnd;
    return fragment.slice(0,at)+namespaceDeclarations(node)+fragment.slice(at);
}
function namespaceWrapper(node) {
    let prefix='rtwrap',serial=0;while(Object.hasOwn(node.namespaces,prefix))prefix='rtwrap'+(++serial);
    return {opening:`<${prefix}:root xmlns:${prefix}="urn:ricewm:text-range-wrapper"`+Object.entries(node.namespaces)
        .filter(([prefix])=>prefix!=='xml').map(([prefix,uri])=>` xmlns${prefix?':'+prefix:''}="${escapeXmlAttribute(uri)}"`).join('')+'>',suffix:`</${prefix}:root>`};
}
function parsedFragment(fragment,context) {
    const {opening,suffix}=namespaceWrapper(context),source=opening+fragment+suffix,document=parseXmlSource(source);
    if(document.root.children.length!==1)fail('ambiguous text fragment');
    return {source,node:document.root.children[0],offset:opening.length,suffix};
}
function setAttributes(source,node,values) {
    const edits=[],missing=[];
    for(const[name,value]of Object.entries(values)) {
        const present=node.attributes.find(a=>a.namespaceURI===''&&a.localName===name);
        if(present)edits.push({start:present.valueStart,end:present.valueEnd,text:escapeXmlAttribute(String(value))});
        else missing.push(` ${name}="${escapeXmlAttribute(String(value))}"`);
    }
    if(missing.length)edits.push({start:node.openEnd-(node.selfClosing?2:1),end:node.openEnd-(node.selfClosing?2:1),text:missing.join('')});
    return patchXmlSource(source,edits);
}
const propertyOrder=['ln','noFill','solidFill','gradFill','blipFill','pattFill','grpFill','effectLst','effectDag','highlight','uLnTx','uLn','uFillTx','uFill','latin','ea','cs','sym','hlinkClick','hlinkMouseOver','rtl','extLst'];
function insertProperty(source,parent,name,xml,namespace) {
    const matches=children(parent,namespace,name);if(matches.length>1)fail('duplicate text character property');
    if(matches.length)return patchXmlSource(source,[{start:matches[0].start,end:matches[0].end,text:xml}]);
    if(parent.selfClosing)return patchXmlSource(source,[{start:parent.openEnd-2,end:parent.openEnd,text:'>'+xml+`</${parent.name}>`}]);
    const rank=propertyOrder.indexOf(name),following=parent.children.find(n=>n.namespaceURI===namespace&&propertyOrder.indexOf(n.localName)>rank);
    const at=following?.start??parent.closeStart;
    return patchXmlSource(source,[{start:at,end:at,text:xml}]);
}
function styleAtom(fragment,atom,style,namespace) {
    let parsed=parsedFragment(fragment,atom.node),properties=children(parsed.node,namespace,'rPr');
    if(properties.length>1)fail('duplicate text run properties');
    if(!properties.length) {
        const tag=`<rtrange:rPr xmlns:rtrange="${namespace}"/>`,node=parsed.node;
        parsed.source=patchXmlSource(parsed.source,[node.selfClosing
            ?{start:node.openEnd-2,end:node.openEnd,text:'>'+tag+`</${node.name}>`}
            :{start:node.openEnd,end:node.openEnd,text:tag}]);
    }
    const reparse=()=>{const doc=parseXmlSource(parsed.source);return children(doc.root.children[0],namespace,'rPr')[0];};
    const values={};
    for(const[key,value]of Object.entries(style)) {
        if(key==='bold')values.b=value?'1':'0';
        if(key==='italic')values.i=value?'1':'0';
        if(key==='underline')values.u=value?'sng':'none';
        if(key==='fontSize')values.sz=Math.round(value*75); // scene px96 -> DrawingML hundredths of a point
    }
    if(Object.keys(values).length)parsed.source=setAttributes(parsed.source,reparse(),values);
    if(style.fontFamily!==undefined)parsed.source=insertProperty(parsed.source,reparse(),'latin',`<rtrange:latin xmlns:rtrange="${namespace}" typeface="${escapeXmlAttribute(style.fontFamily)}"/>`,namespace);
    if(style.color!==undefined) {
        const properties=reparse(),fills=properties.children.filter(n=>n.namespaceURI===namespace&&['noFill','solidFill','gradFill','blipFill','pattFill','grpFill'].includes(n.localName));
        if(fills.length>1)fail('ambiguous text color');
        const color=fills[0]?.children.find(n=>n.namespaceURI===namespace),alpha=color?.children.filter(n=>n.namespaceURI===namespace&&['alpha','alphaMod','alphaOff'].includes(n.localName)).map(n=>movedFragment(parsed.source,n,raw(parsed.source,n))).join('')||'';
        const fill=`<rtrange:solidFill xmlns:rtrange="${namespace}"><rtrange:srgbClr val="${style.color.slice(1).toUpperCase()}">${alpha}</rtrange:srgbClr></rtrange:solidFill>`;
        if(fills.length)parsed.source=patchXmlSource(parsed.source,[{start:fills[0].start,end:fills[0].end,text:fill}]);
        else parsed.source=insertProperty(parsed.source,properties,'solidFill',fill,namespace);
    }
    return parsed.source.slice(parsed.offset,-parsed.suffix.length);
}
function paragraphs(source,body,namespace) {
    const result=[];let base=0;
    for(const node of children(body,namespace,'p')) {
        const atoms=[];let text='';
        for(const item of node.children) {
            if(item.namespaceURI!==namespace)fail('unknown paragraph content');
            if(['pPr','endParaRPr','extLst'].includes(item.localName))continue;
            let value,leaf=null,readonly=false;
            if(['r','fld'].includes(item.localName)) {
                const leaves=children(item,namespace,'t');if(leaves.length!==1)fail('ambiguous text run');
                leaf=leaves[0];value=leafText(source,leaf);readonly=item.localName==='fld';
                if(item.children.some(n=>n.namespaceURI!==namespace||!['rPr','t','extLst'].includes(n.localName)))fail('unsupported text run structure');
            } else if(item.localName==='br')value='\n';
            else fail('unsupported paragraph child');
            const start=encoder.encode(text).length;text+=value;
            atoms.push({node:item,leaf,text:value,start,end:encoder.encode(text).length,readonly});
        }
        const size=encoder.encode(text).length;
        result.push({node,atoms,text,start:base,end:base+size,namespace});base+=size+1;
        if(result.length>256||atoms.length>4096)fail('text structure exceeds bounds');
    }
    return result;
}
function paragraphFormat(row,theme=null){
    const properties=children(row.node,row.namespace,'pPr');if(properties.length>1)fail('duplicate paragraph properties');
    const p=properties[0],number=(name,fallback=0)=>{
        const value=p&&attr(p,name);if(value===undefined)return fallback;
        if(!/^-?\d{1,12}$/u.test(value))fail('invalid paragraph geometry');return Number(value)/9525;
    };
    const level=p&&attr(p,'lvl')!==undefined?Number(attr(p,'lvl')):0;
    if(!Number.isInteger(level)||level<0||level>8)fail('invalid paragraph level');
    let list=null,startAt;
    const bullets=p?p.children.filter(n=>n.namespaceURI===row.namespace&&['buNone','buChar','buAutoNum','buBlip'].includes(n.localName)):[];
    if(bullets.length>1)fail('ambiguous paragraph bullet');
    const bullet=bullets[0];
    if(bullet?.localName==='buNone')list={kind:'none'};
    if(bullet?.localName==='buChar'){
        const glyph=attr(bullet,'char'),style=Object.keys(bulletChars).find(key=>bulletChars[key]===glyph);
        if(style)list={kind:'bullet',style};else if(validBulletGlyph(glyph))list={kind:'bullet',style:'custom',glyph};
    }
    if(bullet?.localName==='buAutoNum'){
        const style=Object.keys(numberSchemes).find(key=>numberSchemes[key]===attr(bullet,'type'));
        if(style){list={kind:'number',style};if(attr(bullet,'startAt')!==undefined){startAt=Number(attr(bullet,'startAt'));if(!Number.isInteger(startAt)||startAt<1||startAt>32767)fail('invalid list starting number');}}
    }
    const marker={};
    if(p){
        const single=names=>{const found=p.children.filter(n=>n.namespaceURI===row.namespace&&names.includes(n.localName));if(found.length>1)fail('ambiguous paragraph marker property');return found[0];};
        const font=single(['buFont','buFontTx']),size=single(['buSzPct','buSzPts','buSzTx']),color=single(['buClr','buClrTx']);
        if(font)marker.fontFamily=font.localName==='buFont'?attr(font,'typeface'):null;
        if(size?.localName==='buSzPct')marker.sizePercent=Number(attr(size,'val'))/1000;
        if(size?.localName==='buSzPts')marker.sizePoints=Number(attr(size,'val'))/100;
        if(color?.localName==='buClrTx')marker.color=null;
        if(color?.localName==='buClr'&&color.children.length===1){
            const child=color.children[0],value=attr(child,'val');
            if(child.namespaceURI===row.namespace&&child.localName==='srgbClr')marker.color='#'+value;
            if(child.namespaceURI===row.namespace&&child.localName==='schemeClr')marker.color=value;
            const resolved=theme?.color(child);if(resolved)marker.resolvedColor={color:resolved.color,opacity:resolved.opacity};
        }
    }
    const knownMarker=Object.keys(marker).length&&validParagraphMarker(marker,{metadata:true});
    return {level,list,marginLeft:number('marL'),marginRight:number('marR'),indent:number('indent'),...(startAt===undefined?{}:{startAt}),...(knownMarker?{marker}:Object.keys(marker).length?{markerUnsupported:true}:{})};
}
function formatParagraphs(source,box,command){
    const selected=textParagraphsInRange(box.text,box.paragraphs,command.range),patches=[];
    if(!selected.length)fail('range contains no source paragraph');
    const startedLevels=new Set();
    for(const row of selected){
        let format=paragraphFormat(row),level=command.format.level??Math.max(0,Math.min(8,format.level+(command.format.levelDelta??0)));
        const list=command.format.list??format.list;
        if((command.format.marker||command.format.startAt!==undefined)&&(!list||list.kind==='none'))fail('marker formatting needs a supported list');
        if(command.format.startAt!==undefined&&list.kind!=='number')fail('starting number needs numbering');
        const node=row.node,properties=children(node,box.namespace,'pPr');
        let fragment=properties.length?movedFragment(source,properties[0],raw(source,properties[0])):`<rplist:pPr xmlns:rplist="${box.namespace}"/>`;
        let parsed=parsedFragment(fragment,node);
        const values={lvl:level};
        // A list gesture owns its hanging indent. Other paragraph geometry,
        // spacing, alignment, defaults, tabs and character properties survive.
        if(level!==format.level)values.marL=Math.max(0,Math.round(format.marginLeft*9525)+457200*(level-format.level));
        if(command.format.list){
            if(list.kind==='none'){values.marL=457200*level;values.indent=0;}
            else if(list.kind==='number'){values.marL=342900+457200*level;values.indent=-342900;}
            else if(format.indent>=0&&format.marginLeft===0){values.marL=228600+457200*level;values.indent=-228600;}
        }
        parsed.source=setAttributes(parsed.source,parsed.node,values);
        const order=['lnSpc','spcBef','spcAft','buClrTx','buClr','buSzTx','buSzPct','buSzPts','buFontTx','buFont','buNone','buAutoNum','buChar','buBlip','tabLst','defRPr','extLst'];
        const replace=(names,xml='')=>{
            parsed.node=parseXmlSource(parsed.source).root.children[0];
            const remove=parsed.node.children.filter(n=>n.namespaceURI===box.namespace&&names.includes(n.localName));
            parsed.source=patchXmlSource(parsed.source,remove.map(n=>({start:n.start,end:n.end,text:''})));
            if(!xml)return;
            parsed.node=parseXmlSource(parsed.source).root.children[0];
            const rank=Math.max(...names.map(n=>order.indexOf(n))),following=parsed.node.children.find(n=>n.namespaceURI===box.namespace&&order.indexOf(n.localName)>rank),at=following?.start??parsed.node.closeStart;
            parsed.source=patchXmlSource(parsed.source,[parsed.node.selfClosing?{start:parsed.node.openEnd-2,end:parsed.node.openEnd,text:'>'+xml+`</${parsed.node.name}>`}:{start:at,end:at,text:xml}]);
        };
        const tag=xml=>xml.replace(/^(<rplist:[^\s/>]+)/u,`$1 xmlns:rplist="${box.namespace}"`);
        if(command.format.list||command.format.startAt!==undefined){
            let start=command.format.startAt===undefined?format.startAt:startedLevels.has(level)?undefined:command.format.startAt;startedLevels.add(level);
            replace(['buNone','buChar','buAutoNum','buBlip'],tag(list.kind==='none'?'<rplist:buNone/>':list.kind==='bullet'?`<rplist:buChar char="${escapeXmlAttribute(list.style==='custom'?list.glyph:bulletChars[list.style])}"/>`:`<rplist:buAutoNum type="${numberSchemes[list.style]}"${start===undefined?'':` startAt="${start}"`}/>`));
            if(list.kind==='none')for(const names of [['buClrTx','buClr'],['buSzTx','buSzPct','buSzPts'],['buFontTx','buFont']])replace(names);
        }
        const marker=command.format.marker;
        if(marker){
            if(Object.hasOwn(marker,'fontFamily'))replace(['buFontTx','buFont'],tag(marker.fontFamily===null?'<rplist:buFontTx/>':`<rplist:buFont typeface="${escapeXmlAttribute(marker.fontFamily)}"/>`));
            if(Object.hasOwn(marker,'sizePercent'))replace(['buSzTx','buSzPct','buSzPts'],tag(`<rplist:buSzPct val="${Math.round(marker.sizePercent*1000)}"/>`));
            if(Object.hasOwn(marker,'color'))replace(['buClrTx','buClr'],tag(marker.color===null?'<rplist:buClrTx/>':`<rplist:buClr><rplist:${marker.color.startsWith('#')?'srgbClr':'schemeClr'} val="${marker.color.startsWith('#')?marker.color.slice(1).toUpperCase():marker.color}"/></rplist:buClr>`));
        }
        fragment=parsed.source.slice(parsed.offset,-parsed.suffix.length);
        let replacement;
        if(properties.length)replacement={start:properties[0].start,end:properties[0].end,text:fragment};
        else replacement=node.selfClosing?{start:node.openEnd-2,end:node.openEnd,text:'>'+fragment+`</${node.name}>`}:{start:node.openEnd,end:node.openEnd,text:fragment};
        patches.push(replacement);
    }
    return patchXmlSource(source,patches);
}
function textBoxes(source,slide) {
    const document=parseXmlSource(source),p=document.root.namespaceURI,a=p===P?A:SA;
    if(![P,SP].includes(p)||document.root.localName!=='sld')fail('invalid slide root');
    const result=new Map(),identities=new Set();
    for(const node of document.nodes.filter(n=>n.namespaceURI===p&&n.localName==='cNvPr')) {
        const identity=attr(node,'id');if(!/^[0-9]{1,10}$/u.test(identity||'')||Number(identity)>0xffffffff)continue;
        if(identities.has(Number(identity)))fail('ambiguous object identity');identities.add(Number(identity));
        const shape=node.parent?.parent;
        if(!shape||shape.namespaceURI!==p)continue;
        if(shape.localName==='graphicFrame'){
            const graphic=children(shape,a,'graphic'),data=graphic.length===1?children(graphic[0],a,'graphicData'):[],tables=data.length===1?children(data[0],a,'tbl'):[];
            if(tables.length!==1)continue;
            const table=tables[0],grid=children(table,a,'tblGrid'),columns=grid.length===1?children(grid[0],a,'gridCol'):[],rows=children(table,a,'tr');
            if(!rows.length||rows.length>128||!columns.length||columns.length>64||rows.length*columns.length>4096)continue;
            if(rows.some(row=>children(row,a,'tc').length!==columns.length))continue;
            for(const[rowIndex,row]of rows.entries())for(const[column,cell]of children(row,a,'tc').entries()){
                if(['hMerge','vMerge'].some(name=>['1','true'].includes(attr(cell,name))))continue;
                const bodies=children(cell,a,'txBody');if(bodies.length!==1)continue;
                let paragraphs_;try{paragraphs_=paragraphs(source,bodies[0],a);}catch{continue;}
                const text=paragraphs_.map(row=>row.text).join('\n');
                if(!paragraphs_.length||encoder.encode(text).length>16384)continue;
                const object=`${slide}/shape:${Number(identity)}`;
                result.set(cellKey(object,rowIndex,column),{body:bodies[0],paragraphs:paragraphs_,text,namespace:a,object,cell:{row:rowIndex,column}});
            }
            continue;
        }
        if(!['sp','cxnSp'].includes(shape.localName))continue;
        const bodies=children(shape,p,'txBody');if(bodies.length!==1)continue;
        let rows;try{rows=paragraphs(source,bodies[0],a);}catch{continue;}
        const text=rows.map(r=>r.text).join('\n');
        if(!rows.length||encoder.encode(text).length>32768)continue;
        result.set(`${slide}/shape:${Number(identity)}`,{body:bodies[0],paragraphs:rows,text,namespace:a});
    }
    return result;
}
function sliceAtom(source,atom,start,end,{move=false}={}) {
    if(start===end)return '';
    let result=raw(source,atom.node);
    if(start!==0||end!==encoder.encode(atom.text).length) {
        if(atom.readonly||!atom.leaf)fail('a text field or break cannot be split');
        const value=decoder.decode(encoder.encode(atom.text).slice(start,end));
        result=patchXmlSource(result,[{start:atom.leaf.start-atom.node.start,end:atom.leaf.end-atom.node.start,text:replaceLeaf(source,atom.leaf,value)}]);
    }
    return move?movedFragment(source,atom.node,result):result;
}
function inheritParagraphCharacters(source,paragraph,atom,fragment) {
    const namespace=paragraph.namespace,properties=children(paragraph.node,namespace,'pPr')[0];
    const defaults=properties&&children(properties,namespace,'defRPr')[0];
    if(!defaults)return fragment;
    let parsed=parsedFragment(fragment,atom.node),node=parsed.node,rPr=children(node,namespace,'rPr')[0];
    if(!rPr) {
        let xml=movedFragment(source,defaults,raw(source,defaults));
        xml=xml.replace(/^<([^\s/>]+):defRPr/u,'<$1:rPr').replace(/<\/([^\s/>]+):defRPr>$/u,'</$1:rPr>');
        if(!defaults.prefix)xml=xml.replace(/^<defRPr/u,'<rPr').replace(/<\/defRPr>$/u,'</rPr>');
        parsed.source=patchXmlSource(parsed.source,[node.selfClosing?{start:node.openEnd-2,end:node.openEnd,text:'>'+xml+`</${node.name}>`}:{start:node.openEnd,end:node.openEnd,text:xml}]);
    } else {
        const missing={};let serial=0;
        for(const attribute of defaults.attributes) {
            if(attribute.namespaceURI===XMLNS||rPr.attributes.some(a=>a.localName===attribute.localName&&a.namespaceURI===attribute.namespaceURI))continue;
            if(!attribute.namespaceURI)missing[attribute.localName]=attribute.value;
            else {
                let prefix;do{prefix='rdinherit'+(++serial);}while(Object.hasOwn(rPr.namespaces,prefix));
                missing['xmlns:'+prefix]=attribute.namespaceURI;missing[prefix+':'+attribute.localName]=attribute.value;
            }
        }
        if(Object.keys(missing).length)parsed.source=setAttributes(parsed.source,rPr,missing);
        const group=name=>['noFill','solidFill','gradFill','blipFill','pattFill','grpFill'].includes(name)?'fill':['effectLst','effectDag'].includes(name)?'effect':['uLnTx','uLn'].includes(name)?'underlineLine':['uFillTx','uFill'].includes(name)?'underlineFill':name;
        for(const child of defaults.children) {
            const doc=parseXmlSource(parsed.source);rPr=children(doc.root.children[0],namespace,'rPr')[0];
            if(rPr.children.some(n=>n.namespaceURI===child.namespaceURI&&group(n.localName)===group(child.localName)))continue;
            parsed.source=insertProperty(parsed.source,rPr,child.localName,movedFragment(source,child,raw(source,child)),child.namespaceURI);
        }
    }
    return parsed.source.slice(parsed.offset,-parsed.suffix.length);
}
function sliceParagraph(source,paragraph,start,end,{move=false}={}) {
    let result='',previous=null;
    for(const atom of paragraph.atoms) {
        const a=Math.max(start,atom.start),b=Math.min(end,atom.end);if(a>=b)continue;
        if(previous)result+=source.slice(previous.node.end,atom.node.start);
        const fragment=sliceAtom(source,atom,a-atom.start,b-atom.start,{move});
        result+=move?inheritParagraphCharacters(source,paragraph,atom,fragment):fragment;previous=atom;
    }
    return result;
}
function paragraphXml(source,paragraph,content) {
    const node=paragraph.node,first=paragraph.atoms[0]?.node,last=paragraph.atoms.at(-1)?.node;
    if(node.selfClosing)return source.slice(node.start,node.openEnd-2)+'>'+content+`</${node.name}>`;
    if(first)return source.slice(node.start,first.start)+content+source.slice(last.end,node.end);
    const end=children(node,paragraph.namespace,'endParaRPr')[0]||children(node,paragraph.namespace,'extLst')[0];
    const at=end?.start??node.closeStart;
    return source.slice(node.start,at)+content+source.slice(at,node.end);
}
function insertionAtom(paragraph,offset,affinity) {
    if(affinity==='before')return [...paragraph.atoms].reverse().find(a=>a.start<offset)||paragraph.atoms[0]||null;
    return paragraph.atoms.find(a=>a.end>offset)||paragraph.atoms.at(-1)||null;
}
function insertionRun(source,paragraph,atom,text) {
    if(atom?.leaf&&!atom.readonly) {
        const fragment=patchXmlSource(raw(source,atom.node),[{start:atom.leaf.start-atom.node.start,end:atom.leaf.end-atom.node.start,text:replaceLeaf(source,atom.leaf,text)}]);
        return movedFragment(source,atom.node,fragment);
    }
    const properties=atom?children(atom.node,paragraph.namespace,'rPr')[0]:null;
    return `<rtrange:r xmlns:rtrange="${paragraph.namespace}">${properties?movedFragment(source,properties,raw(source,properties)):''}<rtrange:t xml:space="preserve">${escaped(text)}</rtrange:t></rtrange:r>`;
}
function locate(rows,position) {
    const index=rows.findIndex(row=>position<=row.end);
    if(index<0)fail('text position is outside the source');
    return {index,row:rows[index],offset:position-rows[index].start};
}
function setParagraphAlignment(source,node,alignment,namespace) {
    const properties=children(node,namespace,'pPr');if(properties.length>1)fail('duplicate paragraph properties');
    if(properties.length)return setAttributes(source,properties[0],{algn:{left:'l',center:'ctr',right:'r'}[alignment]});
    const xml=`<rtrange:pPr xmlns:rtrange="${namespace}" algn="${{left:'l',center:'ctr',right:'r'}[alignment]}"/>`;
    return patchXmlSource(source,[{start:node.openEnd,end:node.openEnd,text:xml}]);
}
function formatRange(source,box,command) {
    const {align,...style}=command.style,patches=[];
    for(const paragraph of box.paragraphs) {
        const first=Math.max(paragraph.start,command.range.start),last=Math.min(paragraph.end,command.range.end);
        if(first>last||first===last&&!(align&&(command.range.start===command.range.end&&command.range.start>=paragraph.start&&command.range.start<=paragraph.end||command.range.start<=paragraph.end&&command.range.end>paragraph.end)))continue;
        let content='';
        if(Object.keys(style).length) {
            let previous=null;
            for(const atom of paragraph.atoms) {
                if(previous)content+=source.slice(previous.node.end,atom.node.start);
                const start=Math.max(0,first-paragraph.start-atom.start),end=Math.min(atom.end-atom.start,last-paragraph.start-atom.start);
                if(start>=end)content+=raw(source,atom.node);
                else {
                    if(atom.readonly)fail('field text must retain its dynamic source');
                    content+=sliceAtom(source,atom,0,start);
                    content+=styleAtom(sliceAtom(source,atom,start,end),atom,style,box.namespace);
                    content+=sliceAtom(source,atom,end,atom.end-atom.start);
                }
                previous=atom;
            }
        }
        let result=Object.keys(style).length?paragraphXml(source,paragraph,content):raw(source,paragraph.node);
        if(align) {
            const parsed=parsedFragment(result,paragraph.node);
            result=setParagraphAlignment(parsed.source,parsed.node,align,box.namespace).slice(parsed.offset,-parsed.suffix.length);
        }
        patches.push({start:paragraph.node.start,end:paragraph.node.end,text:result});
    }
    if(!patches.length)fail('range contains no editable characters');
    return patchXmlSource(source,patches);
}
function replaceRange(source,box,command) {
    if(command.range.start===command.range.end&&!command.insert.length)return source;
    if(!box.paragraphs.length)fail('text body needs a paragraph');
    const first=locate(box.paragraphs,command.range.start),last=locate(box.paragraphs,command.range.end);
    if(box.paragraphs.slice(first.index,last.index+1).some(row=>row.atoms.some(a=>a.readonly&&Math.max(command.range.start,row.start+a.start)<Math.min(command.range.end,row.start+a.end))))fail('field text must retain its dynamic source');
    const atom=insertionAtom(first.row,first.offset,command.affinity);
    let content=sliceParagraph(source,first.row,0,first.offset),result='';
    for(const token of command.insert) {
        if(token.kind==='text'){const fragment=insertionRun(source,first.row,atom,token.text);content+=token.style?styleAtom(fragment,atom||{node:first.row.node},token.style,box.namespace):fragment;}
        else if(token.kind==='softBreak') {
            const properties=atom?children(atom.node,box.namespace,'rPr')[0]:null;
            const fragment=`<rtrange:br xmlns:rtrange="${box.namespace}">${properties?movedFragment(source,properties,raw(source,properties)):''}</rtrange:br>`;
            content+=token.style?styleAtom(fragment,atom||{node:first.row.node},token.style,box.namespace):fragment;
        } else {
            // An empty paragraph still owns the insertion character style.
            // Keep a real empty run, as for the final paragraph below: losing
            // it makes a subsequent native import fall back to master defaults.
            result+=paragraphXml(source,first.row,content||insertionRun(source,first.row,atom,''));content='';
        }
    }
    // Runs retain the namespace bindings of their original source paragraph.
    content+=sliceParagraph(source,last.row,last.offset,last.row.end-last.row.start,{move:first.index!==last.index});
    if(!content)content=insertionRun(source,first.row,atom,'');
    result+=paragraphXml(source,first.row,content);
    return patchXmlSource(source,[{start:first.row.node.start,end:last.row.node.end,text:result}]);
}
function slidePart(parsed,opc,slide) {
    const item=parsed.result.slides.find(s=>s.id===slide);if(!item)fail('unknown slide');
    const p=parseXmlSource(parsed.result.xml).root.namespaceURI,ns=p===P?R:SR;
    const relationship=opc.relationships(parsed.path).find(r=>r.id===item.relationship);
    if(!relationship?.part||relationship.type!==ns+'/slide'||opc.contentType(relationship.part)!=='application/vnd.openxmlformats-officedocument.presentationml.slide+xml')fail('invalid slide relationship');
    return relationship.part;
}

export function inspectPptxTextRanges(input,slide) {
    const parsed=readPptxPackage(input),opc=new OpcPackage(parsed.archive),part=slidePart(parsed,opc,slide);
    let theme=null;try{theme=readPptxSlideTheme(opc,part,parseXmlSource(parsed.result.xml).root.namespaceURI);}catch{/* Unresolved theme metadata does not grant source editability. */}
    return new Map([...textBoxes(decoder.decode(opc.read(part)),slide)].map(([id,box])=>[id,Object.freeze({text:box.text,byteLength:encoder.encode(box.text).length,rangeUnit:'utf8',fields:box.paragraphs.some(p=>p.atoms.some(a=>a.readonly)),paragraphs:box.paragraphs.map(row=>({start:row.start,end:row.end,paragraphFormat:paragraphFormat(row,theme)})),...(box.cell?{object:box.object,cell:{...box.cell}}:{})})]));
}
// Internal shared-codec view. Package identity, source signature and native
// editability remain the caller's responsibility; never expose it as authority.
export {textBoxes as readPptxTextBoxSource};

/** Pure source adapter. The document owner must also admit the original source,
 * import the entire candidate and commit it through the unified history. */
export function editPptxTextRanges(input,commands) {
    if(!Array.isArray(commands)||!commands.length||commands.length>32)fail('expected 1–32 commands');
    const parsed=readPptxPackage(input),opc=new OpcPackage(parsed.archive),changes=new Map();
    for(const rawCommand of commands) {
        const command=checkedTextRangeCommand(rawCommand),part=slidePart(parsed,opc,command.slide),source=changes.get(part)??decoder.decode(opc.read(part));
        if(command.type==='set-paragraph-format'&&command.format.marker?.color&&!command.format.marker.color.startsWith('#')){
            const theme=readPptxSlideTheme(opc,part,parseXmlSource(parsed.result.xml).root.namespaceURI);
            if(!theme.resolveScheme(command.format.marker.color))fail('unresolved marker theme color');
        }
        const key=textRangeTargetKey(command),box=textBoxes(source,command.slide).get(key);if(!box)fail('unknown text box or table cell');
        if(Boolean(command.cell)!==Boolean(box.cell))fail('table cell ranges require an explicit cell target');
        const index=textRangeIndex(box.text);index.scalar(command.range.start);index.scalar(command.range.end);
        const result=command.type==='set-paragraph-format'?formatParagraphs(source,box,command):command.type==='set-text-range-style'?formatRange(source,box,command):replaceRange(source,box,command);
        const after=textBoxes(result,command.slide).get(key);
        const expected=command.type!=='replace-text-range'?box.text:decoder.decode(encoder.encode(box.text).slice(0,command.range.start))+command.insert.map(t=>t.kind==='text'?t.text:'\n').join('')+decoder.decode(encoder.encode(box.text).slice(command.range.end));
        if(!after||after.text!==expected)fail('candidate text differs from the requested range operation');
        changes.set(part,result);
    }
    const replacements=new Map([...changes].map(([part,xml])=>[part,encoder.encode(xml)]));
    return {bytes:parsed.archive.replace(replacements),parts:[...changes.keys()]};
}
inspectPptxTextRanges.controlFactory=Object.freeze({topics:['documents','workflow'],constructor:'inspectPptxTextRanges(bytes,slideId)',methods:'Inspect unsigned PPTX/POTX/PPSX shape and table-cell source ranges. Cell records expose the real object and zero-based cell:{row,column}, keyed through textRangeTargetKey. Fields are marked read-only. Inspection never grants native editability or overrides document/revision guards.'});
editPptxTextRanges.controlFactory=Object.freeze({topics:['documents','workflow'],constructor:'editPptxTextRanges(bytes,commands)',methods:'Prepare source-preserving replace-text-range/set-text-range-style/set-paragraph-format candidates for actual shape IDs or table IDs with cell:{row,column}. Keeps surrounding runs, field source, neighbouring cells and unrelated compressed package records. Uses checked UTF-8 grapheme ranges. Caller validates original native document/table/cell/text editability, imports the complete candidate and commits through one existing history.'});
