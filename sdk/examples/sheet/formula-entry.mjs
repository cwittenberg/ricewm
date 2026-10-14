// Draft-editing helpers only. The Wasm formula parser owns validation/evaluation.
const tail = /(?:^|[^\p{L}\p{N}_.$!])(\$?[A-Za-z]{1,3}\$?[1-9][0-9]{0,6})(?::(\$?[A-Za-z]{1,3}\$?[1-9][0-9]{0,6})?)?$/u;
function cell(address) {
    const [,letters,row]=/^\$?([A-Za-z]+)\$?(\d+)$/u.exec(address);
    let column=0;for(const letter of letters.toUpperCase())column=column*26+letter.charCodeAt(0)-64;
    return {row:Number(row)-1,column:column-1};
}
function quotedAtEnd(text) {
    let quoted=false;
    for(let i=0;i<text.length;i++)if(text[i]==='"'){
        if(quoted&&text[i+1]==='"')i++;else quoted=!quoted;
    }
    return quoted;
}
export function trailingReference(draft,{rows=1048576,columns=16384}={}) {
    if(typeof draft!=='string'||!draft.startsWith('=')||quotedAtEnd(draft))return null;
    const match=tail.exec(draft);if(!match)return null;
    const first=cell(match[1]),last=cell(match[2]||match[1]);
    if([first,last].some(c=>c.row>=rows||c.column>=columns))return null;
    const start=match.index+match[0].indexOf(match[1]);
    return {start,end:draft.length,first,last,colon:draft.endsWith(':')};
}
export function completeFormula(draft) {
    if(typeof draft!=='string'||!draft.startsWith('='))return draft;
    let depth=0,quoted=null;
    for(let i=1;i<draft.length;i++){
        const ch=draft[i];
        if(quoted) {if(ch===quoted){if(draft[i+1]===quoted)i++;else quoted=null;}continue;}
        if(ch==='"'||ch==="'"){quoted=ch;continue;}
        if(ch==='(')depth++;
        if(ch===')'&&--depth<0)return draft;
    }
    // Never invent operands, quotes, range endpoints or function arguments.
    if(quoted||depth<1||depth>64||!/[\p{L}\p{N}_.$%)"}]\s*$/u.test(draft))return draft;
    return draft+')'.repeat(depth);
}

/** Find the replaceable local reference at a native character-based caret.
 * A selected span is replaced verbatim; unrelated suffix text stays intact.
 * Older hosts without selection reporting keep the established trailing path.
 */
export function referenceInsertion(draft,selection,limits={}) {
    if(!selection)return trailingReference(draft,limits)||{start:draft.length,end:draft.length};
    const characters=Array.from(draft),offset=value=>characters.slice(0,Math.max(0,Math.min(characters.length,value))).join('').length;
    const a=offset(selection.anchor),b=offset(selection.caret),start=Math.min(a,b),end=Math.max(a,b);
    if(start!==end)return {start,end};
    const reference=/\$?[A-Za-z]{1,3}\$?[1-9][0-9]{0,6}(?::(?:\$?[A-Za-z]{1,3}\$?[1-9][0-9]{0,6})?)?/uy;
    const identifier=/[\p{L}\p{N}_.$]/u;
    for(let i=1;i<draft.length;){
        if(draft[i]==='"'||draft[i]==="'"){
            const quote=draft[i++];while(i<draft.length){if(draft[i++]===quote){if(draft[i]===quote)i++;else break;}}continue;
        }
        if(draft[i]==='['){while(i<draft.length&&draft[i]!==']')i++;i++;continue;}
        reference.lastIndex=i;const match=!identifier.test(draft[i-1])?reference.exec(draft):null;
        if(!match){i++;continue;}
        const last=reference.lastIndex;
        if((draft[last]&&identifier.test(draft[last]))||/^\s*\(/u.test(draft.slice(last))){i=last;continue;}
        const parsed=trailingReference('='+match[0],limits);
        if(parsed&&start>=i&&start<=last)return {...parsed,start:i,end:last};
        i=last;
    }
    return {start,end};
}
export const characterOffset=(text,index)=>Array.from(text.slice(0,index)).length;
