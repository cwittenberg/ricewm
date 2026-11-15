import {OpcPackage,opcRelationshipsPart,opcPartTarget,OPC_RELATIONSHIPS,OPC_CONTENT_TYPES} from '../opc-package.mjs';
import {parseXmlSource,patchXmlSource,escapeXmlAttribute} from '../xml-source.mjs';
import {readPptxPackage} from './slide-package.mjs';
import {readPptxTableSource} from './table-merge-package.mjs';
import {presentationTableStyles,checkedPresentationTableStyle} from './table-design.mjs';
import {checkedTableDesignCommand} from './table-design-contract.mjs';

const A='http://schemas.openxmlformats.org/drawingml/2006/main',SA='http://purl.oclc.org/ooxml/drawingml/main';
const R='http://schemas.openxmlformats.org/officeDocument/2006/relationships',SR='http://purl.oclc.org/ooxml/officeDocument/relationships';
const TYPE='application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml';
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const flags={headerRow:'firstRow',firstColumn:'firstCol',totalRow:'lastRow',lastColumn:'lastCol',bandedRows:'bandRow',bandedColumns:'bandCol'};
const entries=presentationTableStyles(),ids=new Map(entries.map((entry,index)=>[entry.id,`{52494345-5442-4000-8000-${(index+1).toString(16).toUpperCase().padStart(12,'0')}}`]));
const attr=(node,name)=>node.attributes.find(a=>!a.namespaceURI&&a.localName===name)?.value;
const children=(node,ns,name)=>node.children.filter(n=>n.namespaceURI===ns&&n.localName===name);
const fail=message=>{throw TypeError('Presentation table design source: '+message);};

function definition(entry,namespace){
    const color=(slot,mods=[])=>`<a:schemeClr val="${slot}">${mods.map(([kind,value])=>`<a:${kind} val="${value}"/>`).join('')}</a:schemeClr>`;
    const line=(slot,mods,width)=>'<a:tcBdr>'+['left','right','top','bottom','insideH','insideV'].map(side=>`<a:${side}><a:ln w="${width}"><a:solidFill>${color(slot,mods)}</a:solidFill><a:prstDash val="solid"/></a:ln></a:${side}>`).join('')+'</a:tcBdr>';
    const part=(name,{bold=false,text=null,fill=null,border=null}={})=>`<a:${name}>`+
        (bold||text?`<a:tcTxStyle${bold?' b="on"':''}><a:fontRef idx="minor"><a:prstClr val="black"/></a:fontRef>${color(text??'dk1')}</a:tcTxStyle>`:'')+
        '<a:tcStyle>'+(border?line(...border):'')+(fill?`<a:fill><a:solidFill>${color(...fill)}</a:solidFill></a:fill>`:'')+`</a:tcStyle></a:${name}>`;
    const acc=entry.slot==='tx1'?'dk1':entry.slot,tint=value=>[acc,[['tint',value]]],shade=value=>[acc,[['shade',value]]];
    let parts='';
    switch(entry.family){
    case 'none':parts=part('wholeTbl');break;
    case 'grid':parts=part('wholeTbl',{text:'dk1',border:['dk1',[],9525]});break;
    case 'light1':parts=part('wholeTbl',{text:'dk1'})+part('band1H',{fill:tint(20000)})+part('band1V',{fill:tint(20000)})+
        ['lastCol','firstCol','lastRow'].map(name=>part(name,{bold:true})).join('')+part('firstRow',{bold:true,border:[acc,[],12700]});break;
    case 'light2':parts=part('wholeTbl',{text:'dk1',border:[acc,[],12700]})+part('firstCol',{bold:true})+part('lastRow',{bold:true})+
        part('firstRow',{bold:true,text:'lt1',fill:[acc],border:[acc,[],12700]});break;
    case 'dark1':parts=part('wholeTbl',{text:'lt1',fill:shade(75000),border:['lt1',[],12700]})+part('band1H',{fill:shade(60000)})+part('band1V',{fill:shade(60000)})+
        part('firstCol',{bold:true})+part('lastRow',{bold:true})+part('firstRow',{bold:true,text:'lt1',fill:['dk1']});break;
    case 'medium4':parts=part('wholeTbl',{text:'dk1',fill:tint(20000),border:[acc,[],9525]})+part('band1H',{fill:tint(40000)})+part('band1V',{fill:tint(40000)})+
        ['lastCol','firstCol','lastRow'].map(name=>part(name,{bold:true})).join('')+part('firstRow',{bold:true,fill:tint(40000)});break;
    case 'medium2':parts=part('wholeTbl',{text:'dk1',fill:tint(20000),border:['lt1',[],12700]})+part('band1H',{fill:tint(40000)})+part('band1V',{fill:tint(40000)})+
        ['lastCol','firstCol','lastRow','firstRow'].map(name=>part(name,{bold:true,text:'lt1',fill:[acc]})).join('');break;
    default:fail('unknown style family');
    }
    return `<a:tblStyle xmlns:a="${namespace}" styleId="${ids.get(entry.id)}" styleName="${escapeXmlAttribute(entry.label)}">${parts}</a:tblStyle>`;
}
const semantic=node=>[node.namespaceURI,node.localName,node.attributes.filter(a=>a.namespaceURI!=='http://www.w3.org/2000/xmlns/').map(a=>[a.namespaceURI,a.localName,a.value]).sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b))),node.children.map(semantic)];
const sameDefinition=(node,entry,namespace)=>JSON.stringify(semantic(node))===JSON.stringify(semantic(parseXmlSource(definition(entry,namespace)).root));
function stylesPart(parsed,namespace){
    const opc=new OpcPackage(parsed.archive),rels=opc.relationships(parsed.path),type=(namespace===A?R:SR)+'/tableStyles',matches=rels.filter(rel=>rel.type===type);
    if(matches.length>1)fail('ambiguous table styles relationship');
    if(!matches.length)return {opc,rels,type,path:null,source:null,root:null};
    const rel=matches[0];if(!rel.part||opc.contentType(rel.part)!==TYPE)fail('invalid table styles relationship');
    const source=decoder.decode(opc.read(rel.part)),root=parseXmlSource(source).root;
    if(root.namespaceURI!==namespace||root.localName!=='tblStyleLst')fail('invalid table styles root');
    const seen=new Set();for(const node of children(root,namespace,'tblStyle')){
        const id=attr(node,'styleId');if(!id||seen.has(id.toUpperCase()))fail('duplicate or missing table style identity');seen.add(id.toUpperCase());
    }
    return {opc,rels,type,path:rel.part,source,root};
}
function sourceState(input,slide,object){
    const grid=readPptxTableSource(input,slide,object),parsed=readPptxPackage(input),props=children(grid.table,grid.namespace,'tblPr');
    if(props.length>1)fail('duplicate table properties');
    const pr=props[0],values={};
    for(const[key,name]of Object.entries(flags)){
        const value=pr&&attr(pr,name);if(value!==undefined&&!['0','1','true','false'].includes(value))fail('invalid table option');values[key]=value==='1'||value==='true';
    }
    const styleIds=pr?children(pr,grid.namespace,'tableStyleId'):[];if(styleIds.length>1)fail('duplicate table style reference');
    const node=styleIds[0],styleId=node&&!node.selfClosing?grid.source.slice(node.openEnd,node.closeStart).trim():null;
    const styles=stylesPart(parsed,grid.namespace),matching=styles.root?children(styles.root,grid.namespace,'tblStyle').find(node=>attr(node,'styleId')===styleId):null;
    const entry=entries.find(entry=>ids.get(entry.id)===styleId);
    const design=entry&&matching&&sameDefinition(matching,entry,grid.namespace)?checkedPresentationTableStyle({preset:entry.id,...values}):null;
    return {grid,parsed,pr,styles,design,values,styleId};
}
// Internal source view for the checked semantic projection. No native edit
// capability is inferred from a recognized definition.
export {sourceState as readPptxTableDesignSource};
/** Source state only. Recognition requires the actual style definition as well
 * as its identity; neither an extension marker nor a familiar GUID grants edits. */
export function inspectPptxTableDesign(input,slide,object){
    const {grid,design,values,styleId}=sourceState(input,slide,object);
    return Object.freeze({rows:grid.rows,columns:grid.columns,styleId,design,options:Object.freeze(values)});
}
function append(source,node,fragment){
    return patchXmlSource(source,[node.selfClosing?{start:node.openEnd-2,end:node.openEnd,text:'>'+fragment+`</${node.name}>`}:{start:node.closeStart,end:node.closeStart,text:fragment}]);
}
function changeProperties(source,pr,namespace,style){
    const id=`<a:tableStyleId xmlns:a="${namespace}">${ids.get(style.preset)}</a:tableStyleId>`;
    if(!pr)return `<a:tblPr xmlns:a="${namespace}"${Object.entries(flags).map(([key,name])=>` ${name}="${Number(style[key])}"`).join('')}>${id}</a:tblPr>`;
    let fragment=source.slice(pr.start,pr.end),edits=[],add='';
    for(const[key,name]of Object.entries(flags)){
        const a=pr.attributes.find(a=>!a.namespaceURI&&a.localName===name),value=String(Number(style[key]));
        if(a)edits.push({start:a.valueStart-pr.start,end:a.valueEnd-pr.start,text:value});else add+=` ${name}="${value}"`;
    }
    if(add&&!pr.selfClosing)edits.push({start:pr.openEnd-pr.start-1,end:pr.openEnd-pr.start-1,text:add});
    const existing=children(pr,namespace,'tableStyleId');
    if(existing.length)edits.push({start:existing[0].start-pr.start,end:existing[0].end-pr.start,text:id});
    else if(pr.selfClosing)edits.push({start:pr.openEnd-pr.start-2,end:pr.openEnd-pr.start,text:add+'>'+id+`</${pr.name}>`});
    else {const at=children(pr,namespace,'extLst')[0]?.start??pr.closeStart;edits.push({start:at-pr.start,end:at-pr.start,text:id});}
    return patchXmlSource(fragment,edits);
}
/** Prepare an unsigned source-backed style change without rewriting cells,
 * runs, relationships to other styles or unrelated compressed package members.
 * The document owner must retain native table admission and import this entire
 * candidate before committing a single revision/history action. */
export function editPptxTableDesign(input,{slide,object,style}){
    const requested=checkedTableDesignCommand({type:'set-table-design',slide,object,style}).style,state=sourceState(input,slide,object);
    const checked=checkedPresentationTableStyle({...state.values,...requested}),entry=entries.find(entry=>entry.id===checked.preset);
    const {grid,parsed,pr,styles}=state,changes=new Map(),namespace=grid.namespace,xml=definition(entry,namespace),id=ids.get(entry.id);
    let path=styles.path;
    if(path){
        const match=children(styles.root,namespace,'tblStyle').find(node=>attr(node,'styleId')?.toUpperCase()===id);
        if(match&&!sameDefinition(match,entry,namespace))fail('style identity collides with an existing definition');
        if(!match)changes.set(path,encoder.encode(append(styles.source,styles.root,xml)));
    }else{
        const directory=parsed.path.slice(0,parsed.path.lastIndexOf('/')+1);let index=0;path=directory+'rice-table-styles.xml';
        while(styles.opc.has(path))path=directory+'rice-table-styles-'+(++index)+'.xml';
        changes.set(path,encoder.encode(`<a:tblStyleLst xmlns:a="${namespace}" def="${id}">${xml}</a:tblStyleLst>`));
        const relPath=opcRelationshipsPart(parsed.path),source=styles.opc.has(relPath)?decoder.decode(styles.opc.read(relPath)):`<Relationships xmlns="${OPC_RELATIONSHIPS}"/>`,root=parseXmlSource(source).root;
        let serial=1;while(styles.rels.some(rel=>rel.id==='rIdRiceTableStyles'+serial))serial++;
        changes.set(relPath,encoder.encode(append(source,root,`<Relationship xmlns="${OPC_RELATIONSHIPS}" Id="rIdRiceTableStyles${serial}" Type="${styles.type}" Target="${opcPartTarget(path)}"/>`)));
        const types=decoder.decode(styles.opc.read('[Content_Types].xml'));
        changes.set('[Content_Types].xml',encoder.encode(append(types,parseXmlSource(types).root,`<Override xmlns="${OPC_CONTENT_TYPES}" PartName="${opcPartTarget(path)}" ContentType="${TYPE}"/>`)));
    }
    const properties=changeProperties(grid.source,pr,namespace,checked),at=pr?.start??grid.table.openEnd;
    changes.set(grid.part,encoder.encode(patchXmlSource(grid.source,[{start:at,end:pr?.end??at,text:properties}])));
    const bytes=parsed.archive.edit(changes);const result=inspectPptxTableDesign(bytes,slide,object);
    if(JSON.stringify(result.design)!==JSON.stringify(checked))fail('candidate design did not round trip');
    return Object.freeze({bytes,design:result.design});
}
/** Change only requested table flags, preserving a custom/unknown style ID,
 * its actual definitions and every direct cell/run property. */
export function editPptxTableOptions(input,{slide,object,options}){
    const command=checkedTableDesignCommand({type:'set-table-options',slide,object,options}),state=sourceState(input,slide,object),{grid,pr}=state;
    let xml;
    if(!pr)xml=`<a:tblPr xmlns:a="${grid.namespace}"${Object.entries(command.options).map(([key,value])=>` ${flags[key]}="${Number(value)}"`).join('')}/>`;
    else{
        const edits=[];let add='';
        for(const[key,value]of Object.entries(command.options)){
            const attribute=pr.attributes.find(a=>!a.namespaceURI&&a.localName===flags[key]);
            if(attribute)edits.push({start:attribute.valueStart-pr.start,end:attribute.valueEnd-pr.start,text:String(Number(value))});
            else add+=` ${flags[key]}="${Number(value)}"`;
        }
        if(add){const at=pr.openEnd-pr.start-(pr.selfClosing?2:1);edits.push({start:at,end:at,text:add});}
        xml=patchXmlSource(grid.source.slice(pr.start,pr.end),edits);
    }
    const at=pr?.start??grid.table.openEnd,bytes=grid.archive.replace(new Map([[grid.part,encoder.encode(patchXmlSource(grid.source,[{start:at,end:pr?.end??at,text:xml}]))]]));
    return Object.freeze({bytes,...inspectPptxTableDesign(bytes,slide,object)});
}

/** Internal creation hook. Only the document owner may call this immediately
 * after a successful native insert-table receipt, before any later user edit.
 * Existing/imported tables never pass through this initialization path. */
export function initializeCreatedPptxTableDesign(input,{slide,object}){
    const grid=readPptxTableSource(input,slide,object),edits=[],namespace=grid.namespace;
    if(grid.cells.some(cell=>cell.covered||cell.rowSpan!==1||cell.columnSpan!==1||cell.text!==''))fail('created table is not a pristine empty grid');
    for(const cell of grid.cells){
        const properties=children(cell.node,namespace,'tcPr');if(properties.length!==1)fail('created cell properties are missing');
        for(const node of properties[0].children)if(node.namespaceURI===namespace&&['solidFill','noFill','gradFill','pattFill','blipFill','lnL','lnR','lnT','lnB'].includes(node.localName))edits.push({start:node.start,end:node.end,text:''});
        for(const[name,value]of [['marL','91440'],['marR','91440'],['marT','45720'],['marB','45720']]){
            const a=properties[0].attributes.find(a=>!a.namespaceURI&&a.localName===name);if(!a)fail('created cell margin is missing');edits.push({start:a.valueStart,end:a.valueEnd,text:value});
        }
        for(const paragraph of cell.paragraphs)for(const atom of paragraph.children.filter(n=>n.namespaceURI===namespace&&n.localName==='r')){
            const properties=children(atom,namespace,'rPr');if(properties.length!==1)fail('created character properties are missing');
            const pr=properties[0],bold=pr.attributes.find(a=>!a.namespaceURI&&a.localName==='b'),size=pr.attributes.find(a=>!a.namespaceURI&&a.localName==='sz');
            if(bold)edits.push({start:bold.start,end:bold.end,text:''});
            if(size)edits.push({start:size.valueStart,end:size.valueEnd,text:'1800'});
            for(const node of pr.children){
                if(node.namespaceURI!==namespace)continue;
                if(['solidFill','noFill','gradFill','pattFill','blipFill'].includes(node.localName))edits.push({start:node.start,end:node.end,text:''});
                if(node.localName==='latin'){
                    const face=node.attributes.find(a=>!a.namespaceURI&&a.localName==='typeface');if(!face)fail('created character font is missing');edits.push({start:face.valueStart,end:face.valueEnd,text:'+mn-lt'});
                }
            }
        }
    }
    const bytes=grid.archive.replace(new Map([[grid.part,encoder.encode(patchXmlSource(grid.source,edits))]]));
    return editPptxTableDesign(bytes,{slide,object,style:checkedPresentationTableStyle({preset:'medium2-accent1'})});
}

for(const[name,operation,methods]of [
    ['inspectPptxTableDesign',inspectPptxTableDesign,'Inspect a bounded unsigned source table, six source flags, and the recognized 37-style definition. Unknown styles retain their original identity. This read grants no native capability.'],
    ['editPptxTableDesign',editPptxTableDesign,'Prepare an unsigned PPTX/POTX/PPSX table design candidate with an explicit DrawingML definition and six source options. Omitted options retain actual source values. Preserve direct cell/run properties, existing style definitions, and unrelated compressed members. Caller must verify original native document/table permission, import the complete candidate and commit through its existing revision/history owner.'],
    ['editPptxTableOptions',editPptxTableOptions,'Prepare a partial boolean table-option candidate without replacing the actual custom/unknown style identity, its definitions or cell/run properties. Retains unsigned source identity and the existing caller-owned native admission and history requirements.'],
])operation.controlFactory=Object.freeze({topics:['tables','documents'],constructor:name+'(bytes,...)',methods});
