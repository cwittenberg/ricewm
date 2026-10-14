/** Small presentation adapters; workbook mutations remain engine commands. */
export const SHEET_FONT_FAMILIES=Object.freeze(['Arial','Liberation Sans','DejaVu Sans','DejaVu Serif','DejaVu Sans Mono']);
const FONT_STEPS=Object.freeze([8,9,10,11,12,14,16,18,20,22,24,26,28,36,48,72]);
export function adjacentFontSize(size,direction){
    if(!Number.isFinite(size)||size<1||size>409||![1,-1].includes(direction))return null;
    const next=direction>0?FONT_STEPS.find(value=>value>size)??Math.min(409,size+1):[...FONT_STEPS].reverse().find(value=>value<size)??Math.max(1,size-1);
    return next===size?null:next;
}
/** Preserve literals, color/condition prefixes, percent/currency and scientific
 * exponents. Date/time, fractions and unsupported codes remain untouched. */
export function changeDecimals(format,direction){
    if(typeof format!=='string'||![1,-1].includes(direction))return null;
    if(format.trim().toLowerCase()==='general')return direction>0?'0.0':'0';
    const sections=[];let start=0,quoted=false,bracket=false,mask='';
    for(let i=0;i<format.length;i++){
        const ch=format[i];
        if(ch==='\\'||(!quoted&&!bracket&&(ch==='_'||ch==='*'))){mask+=' ';if(i+1<format.length){mask+=' ';i++;}continue;}
        if(ch==='"'){quoted=!quoted;mask+=' ';continue;}
        if(!quoted&&ch==='['){bracket=true;mask+=' ';continue;}
        if(!quoted&&ch===']'){bracket=false;mask+=' ';continue;}
        if(!quoted&&!bracket&&ch===';'){sections.push({source:format.slice(start,i),mask});start=i+1;mask='';continue;}
        mask+=quoted||bracket?' ':ch;
    }
    if(quoted||bracket)return null;sections.push({source:format.slice(start),mask});
    let changed=false;
    const output=[];
    for(const section of sections){
        if(!/[0#?]/u.test(section.mask)){output.push(section.source);continue;}
        if(/[a-df-z@/]/iu.test(section.mask))return null;
        const match=/[0#?][0#?,]*(?:\.[0#?]*)?/u.exec(section.mask);if(!match)return null;
        const [integer,fraction='']=match[0].split('.'),count=fraction.length,next=count+direction;
        if(next<0){output.push(section.source);continue;}if(next>30)return null;
        const digits=direction>0?fraction+'0':fraction.slice(0,-1);
        output.push(section.source.slice(0,match.index)+integer+(digits?'.'+digits:'')+section.source.slice(match.index+match[0].length));changed=true;
    }
    return changed?output.join(';'):null;
}
