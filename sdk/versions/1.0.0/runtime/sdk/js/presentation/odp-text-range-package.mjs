import {parseXmlSource,patchXmlSource} from '../xml-source.mjs';
import {textRangeIndex} from '../text-ranges.mjs';
import {checkedTextRangeCommand,textRangeTargetKey} from './text-range-contract.mjs';
import {ODF,odfBytes,odfFail,odfRaw,odfSetAttributes,odfAppend,odfParseFragment,readOdpSource,odpSlideIds,odpObjects} from './odp-source.mjs';
import {readOdpBox,odpEncodedText,odpSliceParagraph,odpParagraphXml,odpFormatParagraph,odpInsertionAtom,odpInheritedInsertion} from './odp-text-tree.mjs';
import {OdpTextStyles} from './odp-text-styles.mjs';
import {odpTableTextCells} from './odp-table-text.mjs';

const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
async function slideSource(parsed,slide){const ids=await odpSlideIds(parsed),index=ids.indexOf(slide);if(index<0)odfFail('unknown slide');return{page:parsed.pages[index],index};}
function textTargets(page,slide){
    const result=new Map();
    const add=(id,target)=>{if(result.has(id))odfFail('ambiguous text target identity');result.set(id,target);};
    for(const[id,target]of odpObjects(page,slide)){
        if(target.body)add(id,target);
        for(const[key,cell]of odpTableTextCells(target,id))add(key,cell);
    }
    return result;
}
function checkedBox(parsed,slide,page,command){
    const target=textTargets(page,slide).get(textRangeTargetKey(command));
    if(!target?.body||Boolean(target.cell)!==Boolean(command.cell))odfFail('unknown text box or explicit table cell');
    const box=readOdpBox(parsed.source,target.body);if(target.cell&&odfBytes(box.text)>16384)odfFail('table cell text exceeds bounds');
    return{...box,object:target.node};
}
function location(rows,position){const index=rows.findIndex(row=>position<=row.end);if(index<0)odfFail('range is outside the source text');return{index,row:rows[index],offset:position-rows[index].start};}
function format(parsed,box,command,styles){
    const{align,...characters}=command.style,hasCharacters=Object.keys(characters).length>0,name=hasCharacters?styles.characters(characters):null,patches=[];
    for(const row of box.rows){
        const start=Math.max(row.start,command.range.start),end=Math.min(row.end,command.range.end);
        const paragraphSelected=command.range.start===command.range.end?command.range.start>=row.start&&command.range.start<=row.end:command.range.start<=row.end&&command.range.end>row.start;
        if(start>=end&&!(align&&paragraphSelected))continue;
        let xml=hasCharacters?odpParagraphXml(parsed.source,row,odpFormatParagraph(parsed.source,row,start-row.start,end-row.start,(content,atom)=>styles.wrap(name,content,atom.parent))):odfRaw(parsed.source,row.node);
        if(align){const style=styles.paragraph(row,align),fragment=odfParseFragment(xml,row.node);xml=odfSetAttributes(fragment.source,fragment.node,[[ODF.text,'style-name',style]]).slice(fragment.offset,-fragment.suffix.length);}
        patches.push({start:row.node.start,end:row.node.end,text:xml});
    }
    if(!patches.length)odfFail('range contains no editable characters');return patches;
}
function replacement(parsed,box,command,styles){
    const first=location(box.rows,command.range.start),last=location(box.rows,command.range.end),head=first.row,tail=last.row;
    // Native ODF editability admits only direct paragraphs; preserve that same
    // hierarchy and the original source namespace guard when joining them.
    if(head.node.parent!==tail.node.parent)odfFail('paragraph containers differ');
    const atom=odpInsertionAtom(head,first.offset,command.affinity);
    let content=odpSliceParagraph(parsed.source,head,0,first.offset),result='';
    const retainedComments=box.rows.slice(first.index,last.index+1).flatMap(row=>row.tokens.filter(token=>token.kind==='metadata'&&row.start+token.start>=command.range.start&&row.start+token.start<command.range.end).map(token=>token.raw)).join('');
    content+=retainedComments;
    for(const token of command.insert){
        if(token.kind==='paragraphBreak'){result+=odpParagraphXml(parsed.source,head,content);content='';continue;}
        let xml=token.kind==='softBreak'?odpEncodedText('\n'):odpEncodedText(token.text);
        if(token.style)xml=styles.wrap(styles.characters(token.style),xml,atom?.parent);
        content+=odpInheritedInsertion(parsed.source,head,atom,xml);
        if(content.length+result.length>8*1024*1024)odfFail('replacement source exceeds bounds');
    }
    let suffix=odpSliceParagraph(parsed.source,tail,last.offset,tail.end-tail.start,{includeEnd:true});
    if(first.index!==last.index&&suffix){const inherited=styles.tailInheritance(head,tail);if(inherited)suffix=styles.wrap(inherited,suffix);}
    content+=suffix;result+=odpParagraphXml(parsed.source,head,content);
    return[{start:head.node.start,end:head.node.end,text:result},...box.rows.slice(first.index+1,last.index+1).map(row=>({start:row.node.start,end:row.node.end,text:''}))];
}

/** Inspect exact UTF-8/grapheme text sources in an unsigned ODP/OTP package.
 * The caller must also verify native document/object editability. */
export async function inspectOdpTextRanges(input,slide){
    const parsed=readOdpSource(input),{page,index}=await slideSource(parsed,slide),result=new Map();
    for(const[id,target]of textTargets(page,slide)){
        if(!target.body)continue;
        try{const box=readOdpBox(parsed.source,target.body);if(target.cell&&odfBytes(box.text)>16384)continue;result.set(id,Object.freeze({text:box.text,byteLength:odfBytes(box.text),rangeUnit:'utf8',fields:false,slideIndex:index,...(target.cell?{object:target.object,cell:{...target.cell}}:{})}));}catch{/* Opaque neighboring text never grants an edit capability. */}
    }
    return result;
}

/** Pure source candidate; import it into a disposable native worker and commit
 * through the document owner's existing revision and history transaction. */
export async function editOdpTextRanges(input,commands){
    if(!Array.isArray(commands)||!commands.length||commands.length>32)odfFail('expected 1–32 commands');
    const captured=commands.map(checkedTextRangeCommand);let bytes=input.slice();
    for(const command of captured){
        let parsed=readOdpSource(bytes);const{page}=await slideSource(parsed,command.slide);let box=checkedBox(parsed,command.slide,page,command);
        const index=textRangeIndex(box.text);index.scalar(command.range.start);index.scalar(command.range.end);
        if(command.type==='replace-text-range'&&command.range.start===command.range.end&&!command.insert.length)continue;
        if(!box.rows.length){
            const expanded=odfAppend(parsed.source,box.body,`<rrt:p xmlns:rrt="${ODF.text}"/>`);bytes=parsed.archive.replace(new Map([['content.xml',encoder.encode(expanded)]]));parsed=readOdpSource(bytes);box=checkedBox(parsed,command.slide,(await slideSource(parsed,command.slide)).page,command);
        }
        const styles=new OdpTextStyles(parsed),patches=command.type==='set-text-range-style'?format(parsed,box,command,styles):replacement(parsed,box,command,styles);
        const source=patchXmlSource(parsed.source,[...patches,...styles.patches()]);
        const next=parsed.archive.replace(new Map([['content.xml',encoder.encode(source)]])),afterParsed=readOdpSource(next),after=checkedBox(afterParsed,command.slide,(await slideSource(afterParsed,command.slide)).page,command);
        const text=command.type==='set-text-range-style'?box.text:decoder.decode(encoder.encode(box.text).subarray(0,command.range.start))+command.insert.map(t=>t.kind==='text'?t.text:'\n').join('')+decoder.decode(encoder.encode(box.text).subarray(command.range.end));
        if(after.text!==text)odfFail('candidate text differs from the requested range operation');bytes=next;
    }
    return{bytes,parts:['content.xml']};
}
inspectOdpTextRanges.controlFactory=Object.freeze({topics:['documents','workflow'],constructor:'await inspectOdpTextRanges(bytes,slideId)',methods:'Inspect bounded unsigned ODP/OTP UTF-8 shape and table-cell ranges and native-compatible source identities, including long authored slide IDs. Cell records expose object and cell:{row,column}, keyed through textRangeTargetKey. Repeated/covered cells, unsupported fields, identity-bearing text and opaque structures are not admitted. Caller also checks native document/cell/text editability and the exact revision.'});
editOdpTextRanges.controlFactory=Object.freeze({topics:['documents','workflow'],constructor:'await editOdpTextRanges(bytes,commands)',methods:'Source-preserving ODP/OTP replace-text-range and set-text-range-style candidates, including optional cell:{row,column} on the actual table object. Keeps nested spans, explicit whitespace, paragraph/soft breaks, original style declarations, neighbouring cells and unrelated compressed package records. Never mutates input. Signature/encryption policy is mandatory. Caller imports the whole candidate and commits through one existing document history.'});
