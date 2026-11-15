import {parseXmlSource,patchXmlSource} from './xml-source.mjs';

const P='http://schemas.openxmlformats.org/presentationml/2006/main',SP='http://purl.oclc.org/ooxml/presentationml/main';
const A='http://schemas.openxmlformats.org/drawingml/2006/main',SA='http://purl.oclc.org/ooxml/drawingml/main';
const attr=(node,name)=>node.attributes.find(a=>a.namespaceURI===''&&a.localName===name);
const size=value=>value&&Object.keys(value).sort().join(',')==='height,width'&&Object.values(value).every(n=>Number.isSafeInteger(n)&&n>0&&n<=0x7fffffff);

/** Fit editable DrawingML to a page without rasterizing or introducing a
 * selectable wrapper group. All group coordinate spaces scale together: both
 * ext/chExt and off/chOff change, so nested content scales exactly once.
 * Resource parts scale physical typography/effects, not their own view boxes.
 */
export function fitDrawingmlPart(xml,{sourceSize,destinationSize,resource=false}={}) {
    if(!size(sourceSize)||!size(destinationSize)||typeof resource!=='boolean')throw new TypeError('Expected bounded source and destination page sizes');
    const scale=Math.min(destinationSize.width/sourceSize.width,destinationSize.height/sourceSize.height);
    const offset={x:(destinationSize.width-sourceSize.width*scale)/2,y:(destinationSize.height-sourceSize.height*scale)/2};
    const document=parseXmlSource(xml),root=document.root;
    const strict=root.namespaceURI.startsWith('http://purl.oclc.org/ooxml/'),p=strict?SP:P,a=strict?SA:A;
    if(!resource&&(![P,SP].includes(root.namespaceURI)||!['sld','sldLayout','sldMaster'].includes(root.localName)))throw new TypeError('Expected a DrawingML presentation part');
    if(scale===1&&offset.x===0&&offset.y===0)return Object.freeze({xml,scale,offset:Object.freeze(offset),changed:false});
    const edits=[],insertions=new Map();
    function number(value){if(!/^[+-]?\d+$/u.test(value)||!Number.isSafeInteger(Number(value)))throw new TypeError('Invalid DrawingML physical dimension');return Number(value);}
    function set(node,name,value){const old=attr(node,name);if(old&&old.value===String(value))return;if(old)edits.push({start:old.valueStart,end:old.valueEnd,text:String(value)});else{const at=node.start+1+node.name.length;insertions.set(at,(insertions.get(at)||'')+` ${name}="${value}"`);}}
    function physical(node,name,translation=0,defaultValue){const old=attr(node,name);if(!old&&defaultValue===undefined)return;const value=Math.round((old?number(old.value):defaultValue)*scale+translation);if(!Number.isSafeInteger(value))throw new RangeError('Scaled DrawingML dimension exceeds its bound');set(node,name,value);}
    function opaque(node){for(let owner=node.parent;owner;owner=owner.parent)if([p,a].includes(owner.namespaceURI)&&owner.localName==='extLst')return true;return false;}
    function placeholder(node){const owner=node.parent?.parent;return owner?.children.some(child=>child.namespaceURI===p&&child.localName==='nvSpPr'&&child.children.some(n=>n.namespaceURI===p&&n.localName==='nvPr'&&n.children.some(n=>n.namespaceURI===p&&n.localName==='ph')));}
    for(const node of document.nodes) {
        if(opaque(node))continue;
        if(!resource&&node.namespaceURI===a&&['off','ext','chOff','chExt'].includes(node.localName)&&node.parent?.localName==='xfrm'&&[p,a].includes(node.parent.namespaceURI)) {
            const top=node.parent.parent?.localName==='spPr'?node.parent.parent.parent:node.parent.parent?.localName==='grpSpPr'?node.parent.parent.parent:node.parent.parent;
            const translated=node.localName==='off'&&top?.parent?.namespaceURI===p&&top.parent.localName==='spTree';
            if(node.localName.endsWith('Off')||node.localName==='off'){physical(node,'x',translated?offset.x:0);physical(node,'y',translated?offset.y:0);}
            else{physical(node,'cx');physical(node,'cy');}
        }
        if(node.namespaceURI!==a)continue;
        if(!resource&&node.localName==='gridCol')physical(node,'w');
        if(!resource&&node.localName==='tr')physical(node,'h');
        if(['rPr','defRPr','endParaRPr'].includes(node.localName))for(const name of ['sz','kern','spc'])physical(node,name);
        if(['spcPts','buSzPts'].includes(node.localName))physical(node,'val');
        if(node.localName==='pPr'||/^lvl[1-9]pPr$/u.test(node.localName)||node.localName==='defPPr')for(const name of ['marL','marR','indent','defTabSz'])physical(node,name);
        if(node.localName==='tab')physical(node,'pos');
        if(node.localName==='bodyPr') {
            // A placeholder inherits omitted values. Materialize schema defaults
            // only at the final master or for independent text bodies.
            const defaults=!placeholder(node)||root.localName==='sldMaster';
            for(const[name,value]of [['lIns',91440],['rIns',91440],['tIns',45720],['bIns',45720]])physical(node,name,0,defaults?value:undefined);
            physical(node,'spcCol');
        }
        if(node.localName==='tcPr')for(const[name,value]of [['marL',91440],['marR',91440],['marT',45720],['marB',45720]])physical(node,name,0,value);
        if(['ln','ulLn'].includes(node.localName))physical(node,'w');
        if(['outerShdw','innerShdw','reflection'].includes(node.localName))for(const name of ['blurRad','dist'])physical(node,name);
        if(['glow','softEdge','blur'].includes(node.localName))physical(node,'rad');
        if(['bevelT','bevelB'].includes(node.localName))for(const name of ['w','h'])physical(node,name);
        if(node.localName==='sp3d')for(const name of ['z','extrusionH','contourW'])physical(node,name);
    }
    for(const[at,text]of insertions)edits.push({start:at,end:at,text});
    const result=patchXmlSource(xml,edits);
    return Object.freeze({xml:result,scale,offset:Object.freeze(offset),changed:result!==xml});
}
fitDrawingmlPart.controlFactory=Object.freeze({topics:['documents','visual'],constructor:'fitDrawingmlPart(xml,{sourceSize:{width,height},destinationSize:{width,height},resource:false})',methods:'Return {xml,scale,offset,changed} for a proportional centered page fit in EMUs. Retains editable shapes, group coordinate spaces, inherited layouts, table tracks, typography, insets and physical effects. resource:true scales physical styling in related theme/chart parts without changing their view boxes. Preserves opaque extensions; does not mutate a package, save or commit history.'});
