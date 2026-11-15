import {textRangeIndex} from './text-ranges.mjs';
import {validSceneText} from './presentation/scene-contract.mjs';
import {sourceParagraphs} from './presentation/paragraph-layout.mjs';
import {checkedTextRangeCommand} from './presentation/text-range-contract.mjs';
import {textParagraphsInRange} from './presentation/paragraph-format.mjs';

const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const length=text=>encoder.encode(text).length;
const slice=(text,start,end)=>decoder.decode(encoder.encode(text).subarray(start,end));
const clone=value=>structuredClone(value);
function checked(value){
    if(!validSceneText(value)||length(value.text)>16384||value.runs.length>4096)throw RangeError('Invalid bounded text run draft');
    textRangeIndex(value.text);sourceParagraphs(value,value.runs,style=>style);
    return clone(value);
}
function paragraphs(source){
    let offset=0;
    const runs=source.runs.map(run=>{const start=offset;offset+=length(run.text);return {...run,start,end:offset};});
    return source.paragraphs.map(row=>({...clone(row),text:slice(source.text,row.start,row.end),atoms:runs.flatMap(run=>{
        const start=Math.max(row.start,run.start),end=Math.min(row.end,run.end);
        return start<end?[{text:slice(run.text,start-run.start,end-run.start),style:clone(run.style)}]:[];
    })}));
}
function atomsSlice(atoms,start,end){
    const result=[];let offset=0;
    for(const atom of atoms){const size=length(atom.text),a=Math.max(start,offset),b=Math.min(end,offset+size);if(a<b)result.push({text:slice(atom.text,a-offset,b-offset),style:clone(atom.style)});offset+=size;}
    return result;
}
function rebuild(original,rows){
    const runs=[],paragraphs=[];let text='',at=0;
    for(const[index,row]of rows.entries()){
        const start=at,content=row.atoms.map(atom=>atom.text).join('');
        // Joining paragraphs retains character formatting while every run uses
        // the surviving paragraph's alignment, as the source formats require.
        for(const atom of row.atoms)if(atom.text)runs.push({id:'draft:'+runs.length,text:atom.text,style:{...clone(atom.style),align:row.style.align}});
        at+=length(content);text+=content;
        const {atoms,text:oldText,...metadata}=row;paragraphs.push({...metadata,start,end:at});
        if(index+1<rows.length){runs.push({id:'',text:'\n',style:clone(rows[index+1].style)});at++;text+='\n';}
    }
    const styles=runs.filter(run=>run.id!=='').map(run=>run.style),first=styles[0]||paragraphs[0]?.style||original.style;
    const mixed=styles.some(style=>JSON.stringify(style)!==JSON.stringify(first));
    return checked({...original,text,runs,paragraphs,style:clone(first),mixed,
        unsupported:[...original.unsupported.filter(code=>code!=='MIXED_TEXT_STYLE'),...(mixed?['MIXED_TEXT_STYLE']:[])]});
}

/** A derived editing projection, never a file model or document history owner.
 * Explicit UTF-8 operations retain resolved run styles and distinguish native
 * paragraph boundaries from soft line breaks. The caller applies those same
 * operations to its authoritative source and replaces this projection on read. */
export class TextRunDraft {
    #source;
    static controlFactory=Object.freeze({topics:['documents'],constructor:'new TextRunDraft(nativeSceneText)',methods:'text, snapshot(), paragraphs(range), replace({range,affinity,insert}), style(range,style), styles(range,{affinity:"before"|"after"}). UTF-8/grapheme ranges and explicit text/softBreak/paragraphBreak tokens. Collapsed style queries follow the specified source side. This is a bounded derived projection; the caller owns source transactions, cancellation and Undo/Redo.'});
    constructor(source){this.#source=checked(source);}
    get text(){return this.#source.text;}
    snapshot(){return clone(this.#source);}
    paragraphs(range){return clone(textParagraphsInRange(this.text,this.#source.paragraphs,range));}
    #range(range){const index=textRangeIndex(this.text);if(!range||range.start>range.end)throw RangeError('Invalid draft range');index.scalar(range.start);index.scalar(range.end);}
    replace({range,affinity='before',insert}){
        const command=checkedTextRangeCommand({type:'replace-text-range',slide:'draft',object:'draft',range,affinity,insert});this.#range(range);
        const rows=paragraphs(this.#source),first=rows.findIndex(row=>range.start<=row.end),last=rows.findIndex(row=>range.end<=row.end);
        if(first<0||last<0)throw RangeError('Draft range is outside source paragraphs');
        const head=rows[first],tail=rows[last],offset=range.start-head.start;
        let position=0;const intervals=head.atoms.map(atom=>{const start=position;position+=length(atom.text);return {...atom,start,end:position};});
        const origin=affinity==='before'?[...intervals].reverse().find(atom=>atom.start<offset)||intervals[0]:intervals.find(atom=>atom.end>offset)||intervals.at(-1);
        const style=clone(origin?.style||head.style);
        const replacement=[],makeRow=()=>({...clone(head),atoms:[]});let current=makeRow();
        current.atoms=atomsSlice(head.atoms,0,offset);
        for(const token of command.insert){
            if(token.kind==='paragraphBreak'){replacement.push(current);current=makeRow();}
            else current.atoms.push({text:token.kind==='softBreak'?'\n':token.text,style:{...clone(style),...clone(token.style||{})}});
        }
        current.atoms.push(...atomsSlice(tail.atoms,range.end-tail.start,length(tail.text)));replacement.push(current);
        const source=rebuild(this.#source,[...rows.slice(0,first),...replacement,...rows.slice(last+1)]);
        this.#source=source;return this.snapshot();
    }
    style(range,style){
        checkedTextRangeCommand({type:'set-text-range-style',slide:'draft',object:'draft',range,style});this.#range(range);
        const rows=paragraphs(this.#source),{align,...characters}=style;
        for(const row of rows){
            const a=Math.max(range.start,row.start),b=Math.min(range.end,row.end),collapsed=range.start===range.end&&range.start>=row.start&&range.start<=row.end;
            if(a>b||a===b&&!collapsed&&!(align&&range.start<=row.end&&range.end>row.end))continue;
            if(align){row.style.align=align;for(const atom of row.atoms)atom.style.align=align;}
            if(a<b&&Object.keys(characters).length){
                row.atoms=[...atomsSlice(row.atoms,0,a-row.start),...atomsSlice(row.atoms,a-row.start,b-row.start).map(atom=>({...atom,style:{...atom.style,...characters}})),...atomsSlice(row.atoms,b-row.start,length(row.text))];
            }
        }
        this.#source=rebuild(this.#source,rows);return this.snapshot();
    }
    styles(range,{affinity='before'}={}){
        if(!['before','after'].includes(affinity))throw RangeError('Invalid draft style affinity');
        this.#range(range);const rows=paragraphs(this.#source),result=[];
        for(const row of rows){
            const a=Math.max(range.start,row.start),b=Math.min(range.end,row.end);if(a>b)continue;
            if(a===b&&range.start===range.end){let at=row.start;const atoms=row.atoms.map(atom=>{const start=at;at+=length(atom.text);return {...atom,start,end:at};});const atom=affinity==='before'?[...atoms].reverse().find(atom=>atom.start<a)||atoms[0]:atoms.find(atom=>atom.end>a)||atoms.at(-1);return [clone(atom?.style||row.style)];}
            if(a<b)result.push(...atomsSlice(row.atoms,a-row.start,b-row.start).map(atom=>clone(atom.style)));
        }
        return result;
    }
}
