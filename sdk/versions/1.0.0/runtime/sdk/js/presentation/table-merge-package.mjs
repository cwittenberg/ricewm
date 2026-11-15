import {parseXmlSource, patchXmlSource, escapeXmlAttribute} from '../xml-source.mjs';
import {OpcPackage} from '../opc-package.mjs';
import {readPptxPackage} from './slide-package.mjs';
import {inspectPptxTextRanges} from './text-range-package.mjs';
import {readOdpSource, odpSlideIds, odpObjects, ODF, odfAttribute, odfChildren, odfFragment} from './odp-source.mjs';
import {readOdpBox} from './odp-text-tree.mjs';
import {cellKey} from './table-values.mjs';
import {cellRangeBetween} from '../cell-range.mjs';
import {checkedTableMergeCommand} from './table-merge-contract.mjs';

const P='http://schemas.openxmlformats.org/presentationml/2006/main', SP='http://purl.oclc.org/ooxml/presentationml/main';
const A='http://schemas.openxmlformats.org/drawingml/2006/main', SA='http://purl.oclc.org/ooxml/drawingml/main';
const R='http://schemas.openxmlformats.org/officeDocument/2006/relationships', SR='http://purl.oclc.org/ooxml/officeDocument/relationships';
const encoder=new TextEncoder(), decoder=new TextDecoder('utf-8',{fatal:true});
const children=(node,ns,name)=>node.children.filter(n=>n.namespaceURI===ns&&n.localName===name);
const attr=(node,name)=>node.attributes.find(a=>!a.namespaceURI&&a.localName===name)?.value;
const fail=message=>{throw TypeError('Presentation table merge: '+message);};
const inside=(range,row,column)=>row>=range.firstRow&&row<=range.lastRow&&column>=range.firstColumn&&column<=range.lastColumn;
function positive(value,maximum){if(value===undefined)return 1;if(!/^[0-9]+$/u.test(value)||!Number.isSafeInteger(Number(value))||Number(value)<1||Number(value)>maximum)fail('invalid cell span');return Number(value);}
function validateGrid(grid){
    const {rows,columns,cells}=grid;
    if(!Number.isInteger(rows)||rows<1||rows>128||!Number.isInteger(columns)||columns<1||columns>64||rows*columns>4096||cells.length!==rows*columns)fail('invalid bounded source grid');
    const owners=Array(cells.length).fill(null),merges=[];
    for(const cell of cells){
        if(cell.covered)continue;
        if(cell.row+cell.rowSpan>rows||cell.column+cell.columnSpan>columns)fail('cell span leaves the table');
        const range={firstRow:cell.row,lastRow:cell.row+cell.rowSpan-1,firstColumn:cell.column,lastColumn:cell.column+cell.columnSpan-1};
        if(cell.rowSpan>1||cell.columnSpan>1)merges.push(range);
        for(let r=range.firstRow;r<=range.lastRow;r++)for(let c=range.firstColumn;c<=range.lastColumn;c++){
            const i=r*columns+c,target=cells[i];if(owners[i]!==null)fail('overlapping merged cells');
            if(target!==cell&&!target.covered)fail('merge overlaps a visible cell');
            if(target.covered&&(target.rowSpan!==1||target.columnSpan!==1))fail('covered cells carry independent spans');
            if(grid.format==='pptx'&&(target.horizontal!==(c>cell.column)||target.vertical!==(r>cell.row)))fail('inconsistent horizontal or vertical merge flags');
            owners[i]=cell;
        }
    }
    if(owners.some(owner=>owner===null))fail('orphan covered cell');
    return {...grid,owners,merges};
}
function describe(grid){return Object.freeze({rows:grid.rows,columns:grid.columns,merges:Object.freeze(grid.merges.map(range=>Object.freeze({...range}))),
    cells:Object.freeze(grid.cells.map(({row,column,rowSpan,columnSpan,covered,text})=>Object.freeze({row,column,rowSpan,columnSpan,covered,text})))});}
function pptxGrid(input,slide,object){
    const parsed=readPptxPackage(input),opc=new OpcPackage(parsed.archive),root=parseXmlSource(parsed.result.xml).root;
    const reference=parsed.result.slides.find(row=>row.id===slide);if(!reference)fail('unknown slide');
    const relationship=opc.relationships(parsed.path).find(row=>row.id===reference.relationship),r=root.namespaceURI===P?R:SR;
    if(!relationship?.part||relationship.type!==r+'/slide'||opc.contentType(relationship.part)!=='application/vnd.openxmlformats-officedocument.presentationml.slide+xml')fail('invalid slide source relationship');
    const part=relationship.part,source=decoder.decode(parsed.archive.read(part)),doc=parseXmlSource(source),p=doc.root.namespaceURI,a=p===P?A:SA;
    if(![P,SP].includes(p)||doc.root.localName!=='sld')fail('invalid slide root');
    const seen=new Set();let shape=null;
    for(const node of doc.nodes.filter(n=>n.namespaceURI===p&&n.localName==='cNvPr')){
        const id=attr(node,'id');if(!/^[0-9]{1,10}$/u.test(id||'')||Number(id)>0xffffffff)fail('invalid object identity');
        if(seen.has(Number(id)))fail('duplicate object identity');seen.add(Number(id));
        if(`${slide}/shape:${Number(id)}`===object){const candidate=node.parent?.parent;if(candidate?.namespaceURI===p&&candidate.localName==='graphicFrame'&&node.parent.localName==='nvGraphicFramePr')shape=candidate;}
    }
    if(!shape)fail('unknown source table');
    const graphics=children(shape,a,'graphic'),data=graphics.length===1?children(graphics[0],a,'graphicData'):[],tables=data.length===1?children(data[0],a,'tbl'):[];
    if(tables.length!==1)fail('ambiguous table source');
    const table=tables[0],grids=children(table,a,'tblGrid'),columns=grids.length===1?children(grids[0],a,'gridCol'):[],rows=children(table,a,'tr');
    const ranges=inspectPptxTextRanges(input,slide),cells=[];
    for(const[row,tr]of rows.entries()){
        const list=children(tr,a,'tc');if(list.length!==columns.length)fail('nonrectangular table source');
        for(const[column,node]of list.entries()){
            const horizontal=['1','true'].includes(attr(node,'hMerge')),vertical=['1','true'].includes(attr(node,'vMerge')),covered=horizontal||vertical;
            for(const flag of ['hMerge','vMerge'])if(attr(node,flag)!==undefined&&!['0','1','true','false'].includes(attr(node,flag)))fail('invalid merge flag');
            const bodies=children(node,a,'txBody');if(bodies.length!==1)fail('missing unique cell text body');
            const body=bodies[0],paragraphs=children(body,a,'p'),range=ranges.get(cellKey(object,row,column));
            if(!covered&&(!range||range.fields))fail('unsupported cell text');
            if(covered&&doc.nodes.some(n=>n.namespaceURI===a&&n.localName==='t'&&n.start>=body.start&&n.end<=body.end&&!n.selfClosing&&source.slice(n.openEnd,n.closeStart)!==''))fail('covered cell contains hidden text');
            cells.push({row,column,rowSpan:positive(attr(node,'rowSpan'),128),columnSpan:positive(attr(node,'gridSpan'),64),horizontal,vertical,covered,node,body,paragraphs,text:range?.text||''});
        }
    }
    return validateGrid({format:'pptx',archive:parsed.archive,part,source,table,namespace:a,rows:rows.length,columns:columns.length,cells});
}
async function odpGrid(input,slide,object){
    const parsed=readOdpSource(input),ids=await odpSlideIds(parsed),index=ids.indexOf(slide);if(index<0)fail('unknown slide');
    const target=odpObjects(parsed.pages[index],slide).get(object);if(!target)fail('unknown table object');
    const tables=odfChildren(target.node,ODF.table,'table');if(tables.length!==1)fail('ambiguous table source');
    const table=tables[0],nodes=[];
    function descend(node){for(const child of node.children){if(child.namespaceURI!==ODF.table||child.localName==='table')continue;nodes.push(child);descend(child);}}
    descend(table);if(nodes.length>20000)fail('table source exceeds bounds');
    for(const node of nodes)for(const key of ['number-rows-repeated','number-columns-repeated'])if(positive(odfAttribute(node,ODF.table,key),4096)!==1)fail('repeated source cells retain their native read-only guard');
    const rows=nodes.filter(n=>n.localName==='table-row'),columns=nodes.filter(n=>n.localName==='table-column'),cells=[];
    for(const[row,tr]of rows.entries()){
        const list=tr.children.filter(n=>n.namespaceURI===ODF.table&&['table-cell','covered-table-cell'].includes(n.localName));if(list.length!==columns.length)fail('nonrectangular source table');
        for(const[column,node]of list.entries()){
            const covered=node.localName==='covered-table-cell',box=readOdpBox(parsed.source,node);
            if(covered&&box.text!=='')fail('covered cell contains hidden text');
            cells.push({row,column,rowSpan:positive(odfAttribute(node,ODF.table,'number-rows-spanned'),128),columnSpan:positive(odfAttribute(node,ODF.table,'number-columns-spanned'),64),covered,node,body:node,paragraphs:box.rows.map(row=>row.node),text:box.text});
        }
    }
    return validateGrid({format:'odp',archive:parsed.archive,part:'content.xml',source:parsed.source,table,namespace:ODF.text,rows:rows.length,columns:columns.length,cells});
}
export function inspectPptxTableMerges(input,slide,object){return describe(pptxGrid(input,slide,object));}
// Shared source codecs reuse the same unsigned identity, unique object and
// complete bounded grid checks. This internal view grants no native permission.
export {pptxGrid as readPptxTableSource};
export async function inspectOdpTableMerges(input,slide,object){return describe(await odpGrid(input,slide,object));}
function attributes(source,node,values){
    const edits=[],additions=[];
    for(const[namespace,name,value]of values){
        const attribute=node.attributes.find(a=>a.namespaceURI===namespace&&a.localName===name);
        if(attribute){edits.push(value===null?{start:attribute.start,end:attribute.end,text:''}:{start:attribute.valueStart,end:attribute.valueEnd,text:escapeXmlAttribute(String(value))});continue;}
        if(value===null)continue;
        const prefix=namespace?Object.entries(node.namespaces).find(([prefix,ns])=>prefix&&ns===namespace)?.[0]:'';
        if(namespace&&!prefix)fail('source cell has no namespace prefix');
        additions.push(` ${prefix?prefix+':':''}${name}="${escapeXmlAttribute(String(value))}"`);
    }
    if(additions.length)edits.push({start:node.openEnd-(node.selfClosing?2:1),end:node.openEnd-(node.selfClosing?2:1),text:additions.join('')});
    return edits;
}
function replaceParagraphs(grid,cell,content){
    const {source,format,namespace}=grid,body=cell.body,paragraphs=cell.paragraphs;
    if(content===null)content=`<rrm:p xmlns:rrm="${namespace}"/>`;
    if(paragraphs.length)return paragraphs.map((p,i)=>({start:p.start,end:p.end,text:i?'':content}));
    if(format==='odp'&&body.selfClosing)return [{start:body.openEnd-2,end:body.openEnd,text:'>'+content+`</${body.name}>`}];
    return [{start:body.closeStart,end:body.closeStart,text:content}];
}
function editGrid(grid,command){
    const {source,rows,columns,cells}=grid,edits=[];let selected,anchor,range;
    if(command.type==='merge-table-cells'){
        range=cellRangeBetween(command.from,command.to,{rows,columns,merges:grid.merges});
        anchor=cells[range.firstRow*columns+range.firstColumn];
        selected=cells.filter(cell=>inside(range,cell.row,cell.column));
        if(selected.length<2)fail('select more than one cell to merge');
        const paragraphs=selected.filter(cell=>!cell.covered&&cell.text!=='').flatMap(cell=>cell.paragraphs.map(p=>odfFragment(source,p))).join('');
        if(paragraphs)edits.push(...replaceParagraphs(grid,anchor,paragraphs));
        for(const cell of selected)if(cell!==anchor&&!(grid.format==='odp'&&cell.body.selfClosing))edits.push(...replaceParagraphs(grid,cell,null));
    }else{
        anchor=cells[command.row*columns+command.column];
        if(!anchor||anchor.covered||anchor.rowSpan===1&&anchor.columnSpan===1)fail('select a merged anchor cell to split');
        range={firstRow:anchor.row,lastRow:anchor.row+anchor.rowSpan-1,firstColumn:anchor.column,lastColumn:anchor.column+anchor.columnSpan-1};
        selected=cells.filter(cell=>inside(range,cell.row,cell.column));
    }
    for(const cell of selected){
        const merging=command.type==='merge-table-cells',isAnchor=cell===anchor;
        const rowSpan=merging&&isAnchor?range.lastRow-range.firstRow+1:1,columnSpan=merging&&isAnchor?range.lastColumn-range.firstColumn+1:1;
        if(grid.format==='pptx')edits.push(...attributes(source,cell.node,[['','rowSpan',rowSpan>1?rowSpan:null],['','gridSpan',columnSpan>1?columnSpan:null],
            ['','hMerge',merging&&cell.column>anchor.column?'1':null],['','vMerge',merging&&cell.row>anchor.row?'1':null]]));
        else{
            const covered=merging&&!isAnchor,name=cell.node.prefix?cell.node.prefix+':':'';
            edits.push(...attributes(source,cell.node,[[ODF.table,'number-rows-spanned',rowSpan>1?rowSpan:null],[ODF.table,'number-columns-spanned',columnSpan>1?columnSpan:null]]));
            if((cell.node.localName==='covered-table-cell')!==covered){
                const desired=name+(covered?'covered-table-cell':'table-cell');
                edits.push({start:cell.node.start+1,end:cell.node.start+1+cell.node.name.length,text:desired});
                if(!cell.node.selfClosing)edits.push({start:cell.node.closeStart+2,end:cell.node.closeStart+2+cell.node.name.length,text:desired});
            }
        }
    }
    const xml=patchXmlSource(source,edits),bytes=grid.archive.replace(new Map([[grid.part,encoder.encode(xml)]]));
    return {bytes,range:Object.freeze({...range})};
}
export function editPptxTableMerges(input,commands){
    if(!Array.isArray(commands)||!commands.length||commands.length>32)fail('expected 1–32 table commands');const captured=commands.map(checkedTableMergeCommand);let bytes=input.slice(),range;
    for(const command of captured){({bytes,range}=editGrid(pptxGrid(bytes,command.slide,command.object),command));pptxGrid(bytes,command.slide,command.object);}
    return Object.freeze({bytes,range});
}
export async function editOdpTableMerges(input,commands){
    if(!Array.isArray(commands)||!commands.length||commands.length>32)fail('expected 1–32 table commands');const captured=commands.map(checkedTableMergeCommand);let bytes=input.slice(),range;
    for(const command of captured){({bytes,range}=editGrid(await odpGrid(bytes,command.slide,command.object),command));await odpGrid(bytes,command.slide,command.object);}
    return Object.freeze({bytes,range});
}
for(const [name,operation,format]of [['inspectPptxTableMerges',inspectPptxTableMerges,'PPTX/POTX/PPSX'],['inspectOdpTableMerges',inspectOdpTableMerges,'ODP/OTP']])operation.controlFactory=Object.freeze({topics:['documents'],constructor:`${name}(bytes,slideId,objectId)`,methods:`Inspect a bounded unsigned ${format} table's exact source cells and coherent merge topology. No document mutation or native editability grant. Unknown, signed, encrypted, repeated, overlapping or inconsistent source is rejected.`});
for(const[name,operation,format]of [['editPptxTableMerges',editPptxTableMerges,'PPTX/POTX/PPSX'],['editOdpTableMerges',editOdpTableMerges,'ODP/OTP']])operation.controlFactory=Object.freeze({topics:['documents'],constructor:`${name}(bytes,commands)`,methods:`Prepare source-preserving ${format} merge-table-cells or split-table-cell bytes. Appends nonempty source paragraphs in row-major order, retains rich runs and cell properties, and keeps unrelated compressed package records. Caller must verify native document and cell editability and import the entire candidate before committing through one existing history.`});
