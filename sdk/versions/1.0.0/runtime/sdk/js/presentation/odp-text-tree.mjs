import {ODF,odfAttribute,odfBytes,odfEscape,odfFail,odfRaw} from './odp-source.mjs';

const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
export const odpTextSlice=(value,start,end)=>decoder.decode(encoder.encode(value).subarray(start,end));
function decode(value){return value.replace(/&([^;]+);/gu,(_,name)=>({amp:'&',lt:'<',gt:'>',quot:'"',apos:"'"}[name]??String.fromCodePoint(name.startsWith('#x')?parseInt(name.slice(2),16):Number(name.slice(1))))).replaceAll('\r\n','\n').replaceAll('\r','\n');}

/** ODF 1.3 6.1.2: normalize ordinary whitespace across spans, independently
 * of explicit text:s, text:tab and text:line-break. Lexical comments survive. */
export function readOdpBox(source,body){
    if(!body)odfFail('missing text body');
    const rows=[];let bytes=0,atoms=0;
    for(const node of body.children){
        if(node.namespaceURI===ODF.draw&&node.localName==='enhanced-geometry')continue;
        if(node.namespaceURI!==ODF.text||!['p','h'].includes(node.localName))odfFail('unsupported text body structure');
        if(node.attributes.some(a=>a.namespaceURI===ODF.xml&&a.localName==='id'))odfFail('identity-bearing paragraphs retain their native edit guard');
        const tokens=[];
        function gap(start,end,parent){
            const result=[],value=source.slice(start,end),pattern=/<!--[\s\S]*?-->|<!\[CDATA\[([\s\S]*?)\]\]>|<\?[\s\S]*?\?>|[^<]+/gu;let at=0;
            for(const match of value.matchAll(pattern)){
                if(match.index!==at)odfFail('unrecognized inline markup');at=match.index+match[0].length;
                const markup=match[0].startsWith('<')&&match[1]===undefined;
                const item=markup?{kind:'metadata',raw:match[0],parent}:{kind:'ordinary',text:match[1]!==undefined?match[1].replaceAll('\r\n','\n').replaceAll('\r','\n'):decode(match[0]),parent};
                result.push(item);tokens.push(item);if(++atoms>4096)odfFail('too many text atoms');
            }
            if(at!==value.length)odfFail('unrecognized inline source');return result;
        }
        function tree(element){
            if(element.attributes.some(a=>a.namespaceURI===ODF.xml&&a.localName==='id'))odfFail('identity-bearing spans retain their native edit guard');
            const result={kind:'container',node:element,children:[]};let at=element.openEnd;
            for(const child of element.children){
                result.children.push(...gap(at,child.start,element));at=child.end;
                if(child.namespaceURI!==ODF.text)odfFail('unsupported inline source');
                if(['span','a'].includes(child.localName))result.children.push(tree(child));
                else if(['s','tab','line-break'].includes(child.localName)){
                    if(child.children.length||!child.selfClosing&&source.slice(child.openEnd,child.closeStart)!=='')odfFail('explicit whitespace must have empty content');
                    let text=child.localName==='s'?' ':child.localName==='tab'?'\t':'\n';
                    if(child.localName==='s'){
                        const count=odfAttribute(child,ODF.text,'c')??'1';if(!/^[0-9]+$/u.test(count)||!Number.isSafeInteger(Number(count))||Number(count)<1||Number(count)>32768)odfFail('invalid explicit space count');text=' '.repeat(Number(count));
                    }
                    const item={kind:'explicit',node:child,text,parent:element};result.children.push(item);tokens.push(item);if(++atoms>4096)odfFail('too many text atoms');
                }else odfFail('fields and unsupported inline source are not editable');
            }
            if(!element.selfClosing)result.children.push(...gap(at,element.closeStart,element));
            return result;
        }
        const root=tree(node);let begun=false,space=false;
        for(const token of tokens){
            if(token.kind==='metadata')continue;
            if(token.kind==='explicit'){begun=true;space=false;continue;}
            token.text=token.text.replace(/[ \t\r\n]+/gu,' ');
            let value='';for(const character of token.text){if(character===' '){if(!begun||space)continue;space=true;}else{begun=true;space=false;}value+=character;}token.text=value;
        }
        for(const token of [...tokens].reverse()){
            if(token.kind==='metadata')continue;if(token.kind==='explicit')break;
            token.text=token.text.replace(/ +$/u,'');if(token.text)break;
        }
        let offset=0;
        for(const token of tokens){token.start=offset;if(token.kind!=='metadata')offset+=odfBytes(token.text);token.end=offset;}
        const text=tokens.filter(t=>t.kind!=='metadata').map(t=>t.text).join('');
        rows.push({node,root,tokens,text,start:bytes,end:bytes+offset});bytes+=offset+1;
        if(rows.length>256||bytes>32769)odfFail('text structure exceeds bounds');
    }
    if(rows.length){const scope=node=>JSON.stringify(Object.entries(node.namespaces).sort(([a],[b])=>a.localeCompare(b)));if(rows.some(row=>scope(row.node)!==scope(rows[0].node)))odfFail('paragraph namespace contexts differ');}
    return{body,rows,text:rows.map(row=>row.text).join('\n')};
}

export function odpEncodedText(value){
    let result='',ordinary='';const flush=()=>{result+=odfEscape(ordinary);ordinary='';};
    for(let at=0;at<value.length;){
        const c=value[at];
        if(c===' '){flush();let end=at+1;while(value[end]===' ')end++;result+=`<rrt:s xmlns:rrt="${ODF.text}"${end-at>1?` rrt:c="${end-at}"`:''}/>`;at=end;}
        else if(c==='\t'||c==='\n'){flush();result+=`<rrt:${c==='\t'?'tab':'line-break'} xmlns:rrt="${ODF.text}"/>`;at++;}
        else{ordinary+=c;at++;}
    }
    flush();return result;
}
function container(source,node,content){
    if(node.selfClosing)return content?source.slice(node.start,node.openEnd-2)+'>'+content+`</${node.name}>`:odfRaw(source,node);
    return source.slice(node.start,node.openEnd)+content+source.slice(node.closeStart,node.end);
}
export function odpParagraphXml(source,row,content){return container(source,row.node,content);}
export function odpSliceParagraph(source,row,start,end,{includeEnd=false}={}){
    function visit(item){
        if(item.kind==='container'){
            const content=item.children.map(visit).join('');return content?container(source,item.node,content):'';
        }
        if(item.kind==='metadata')return item.start>=start&&(item.start<end||includeEnd&&item.start===end)?item.raw:'';
        const a=Math.max(start,item.start),b=Math.min(end,item.end);return a<b?odpEncodedText(odpTextSlice(item.text,a-item.start,b-item.start)):'';
    }
    return row.root.children.map(visit).join('');
}
export function odpFormatParagraph(source,row,start,end,style){
    function visit(item){
        if(item.kind==='container')return container(source,item.node,item.children.map(visit).join(''));
        if(item.kind==='metadata')return item.raw;
        const a=Math.max(start,item.start),b=Math.min(end,item.end);
        if(a>=b)return odpEncodedText(item.text);
        return odpEncodedText(odpTextSlice(item.text,0,a-item.start))+style(odpEncodedText(odpTextSlice(item.text,a-item.start,b-item.start)),item)+odpEncodedText(odpTextSlice(item.text,b-item.start,odfBytes(item.text)));
    }
    return row.root.children.map(visit).join('');
}
export function odpInsertionAtom(row,offset,affinity){
    const atoms=row.tokens.filter(t=>t.kind!=='metadata'&&t.text);
    return affinity==='before'?[...atoms].reverse().find(t=>t.start<offset)||atoms[0]:atoms.find(t=>t.end>offset)||atoms.at(-1);
}
export function odpInheritedInsertion(source,row,atom,content){
    let parent=atom?.parent;
    while(parent&&parent!==row.node){
        content=container(source,parent,content);parent=parent.parent;
    }
    return content;
}
