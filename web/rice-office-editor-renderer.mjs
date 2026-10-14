import {officeFace, officeFamilyFace, officeCanvasFont} from './rice-office-fonts.mjs';
import {editorGlyphAdvance} from "./rice-editor-font-metrics.mjs";
// Office layout is authored by the native document model. No imported HTML or
// XML is inserted into the DOM. Only bundled font assets are loaded; imported
// document URLs never trigger font or resource requests.
export function parseOfficeLayout(language) {
    const header=/(?:^|~)o(\d+),(\d+),(\d+),(\d+)(?:~|$)/.exec(language);
    if(!header)return null;
    const [width,height,scroll,pages]=header.slice(1).map(Number);
    if(width<200||width>4000||height<200||height>6000||!Number.isSafeInteger(scroll)||pages>100000)return null;
    const lines=[],cells=[],styles=[],markers=[],runs=[],stops=[],clusters=[];
    const offset=n=>Number.isSafeInteger(n)&&n>=0&&n<=1000000;
    const geometry=n=>Number.isFinite(n)&&Math.abs(n)<=1000000000;
    let editingLine=null;
    for(const token of language.split('~')) {
        const type=token[0],parts=token.slice(1).split(',');
        if(type==='e') {
            const [y0,y1]=parts.map(Number);
            editingLine=parts.length===2&&[y0,y1].every(geometry)&&y1>y0?{y0,y1}:null;
        } else if(type==='d'&&editingLine) {
            const [at,x,affinity]=parts.map(Number);
            if(parts.length===3&&offset(at)&&geometry(x)&&[0,1].includes(affinity))stops.push({offset:at,x,...editingLine,affinity});
        } else if(type==='a'&&editingLine) {
            const [start,end,x0,x1]=parts.map(Number);
            if(parts.length===4&&offset(start)&&offset(end)&&start<=end&&[x0,x1].every(geometry)&&x1>=x0)clusters.push({start,end,x0,x1,...editingLine});
        } else if(type==='q') {
            const [start,end,x,baseline,height,advance,size,face,rtl]=parts.slice(0,9).map(Number);
            if(parts.length===10&&offset(start)&&offset(end)&&start<=end&&[x,baseline,height,advance,size].every(geometry)&&height>0&&advance>=0&&size>=6&&size<=128&&Number.isInteger(face)&&officeFace(face)&&[0,1].includes(rtl))
                runs.push({start,end,x,baseline,height,advance,size,face,rtl:!!rtl,color:/^[0-9a-f]{6}$/i.test(parts[9])?'#'+parts[9]:'#202124'});
        } else if(type==='t') {
            const [at,x,y0,y1,affinity]=parts.map(Number);
            if(parts.length===5&&offset(at)&&[x,y0,y1].every(geometry)&&y1>y0&&[0,1].includes(affinity))stops.push({offset:at,x,y0,y1,affinity});
        } else if(type==='k') {
            const [start,end,x0,x1,y0,y1]=parts.map(Number);
            if(parts.length===6&&offset(start)&&offset(end)&&start<=end&&[x0,x1,y0,y1].every(geometry)&&x1>=x0&&y1>y0)clusters.push({start,end,x0,x1,y0,y1});
        } else if(type==='l') {
            const [start,end,x,y,size]=parts.slice(0,5).map(Number);
            if([start,end,x,y,size].every(Number.isSafeInteger)&&start<=end&&size>=6&&size<=128)lines.push({start,end,x,y,size,font:decode(parts[5]),color:/^[0-9a-f]{6}$/i.test(parts[6])?'#'+parts[6]:'#202124'});
        } else if(type==='c') {
            const rect=parts.map(Number);if(rect.length===4&&rect.every(n=>Number.isSafeInteger(n)&&n>=0))cells.push(rect);
        } else if(type==='m') {
            const [x,y,size]=parts.slice(0,3).map(Number),text=decode(parts[5]);
            if(parts.length===6&&[x,y,size].every(Number.isSafeInteger)&&x>=0&&x<=width&&y>=0&&y<=(height+24)*pages&&size>=6&&size<=128&&text&&!/[\x00-\x1f\x7f]/.test(text)) {
                markers.push({x,y,size,font:decode(parts[3]),color:/^[0-9a-f]{6}$/i.test(parts[4])?'#'+parts[4]:'#202124',text});
            }
        } else {
            const match=/^([biush])(\d+)-(\d+)(?:,([a-f0-9]{6}|none))?$/.exec(token);
            if(match)styles.push({kind:match[1],start:Number(match[2]),end:Number(match[3]),value:match[4]});
        }
    }
    return {width,height,scroll,pages,lines,cells,styles,markers,runs,stops,clusters,caretAffinity:language.split('~').includes('E0')?0:1,printLayout:!language.split('~').includes('P0')};
}
function decode(hex='') {
    if(hex.length>512||!/^(?:[a-f0-9]{2})*$/i.test(hex))return '';
    return new TextDecoder().decode(Uint8Array.from(hex.match(/../g)||[],b=>parseInt(b,16)));
}
export function paintOfficeEditor(context,command,presentation) {
    const layout=parseOfficeLayout(presentation.language);if(!layout)return false;
    const {rect}=command,scale=Math.min(1,Math.max(1,rect.width-16)/layout.width)*(presentation.fontSize||12)/12,inset=8;
    const chars=Array.from(presentation.document),caret=presentation.caret??0,anchor=presentation.anchor??caret;
    const pageLeft=Math.max(inset,(rect.width-layout.width*scale)/2);
    const x=value=>rect.x+pageLeft+value*scale,y=value=>rect.y+inset+(value-layout.scroll)*scale;
    context.save();context.beginPath();context.rect(rect.x,rect.y,rect.width,rect.height);context.clip();
    context.fillStyle=layout.printLayout?'#d7dbe1':'#ffffff';context.fillRect(rect.x,rect.y,rect.width,rect.height);
    const stride=layout.height+24;
    const first=Math.max(0,Math.floor(layout.scroll/stride));
    const last=Math.min(layout.pages-1,Math.ceil((layout.scroll+rect.height/scale)/stride));
    for(let page=first;layout.printLayout&&page<=last;page++) {
        context.fillStyle='#ffffff';context.fillRect(x(0),y(page*stride),layout.width*scale,layout.height*scale);
        context.strokeStyle='#aeb4bd';context.lineWidth=1;context.strokeRect(x(0),y(page*stride),layout.width*scale,layout.height*scale);
    }
    context.strokeStyle='#747b86';context.lineWidth=Math.max(.5,scale);
    for(const [cx,cy,cw,ch] of layout.cells)context.strokeRect(x(cx),y(cy),cw*scale,ch*scale);
    context.textBaseline='top';context.textAlign='left';
    // Labels are separate paint decorations. They never consume editable text
    // positions, selection highlights, clipboard characters or caret locations.
    for(const marker of layout.markers) {
        const top=y(marker.y),height=marker.size*1.5*scale;
        if(top+height<rect.y||top>rect.y+rect.height)continue;
        context.font=officeCanvasFont(officeFamilyFace(marker.font),marker.size*scale);
        const body=layout.lines.find(l=>l.y===marker.y&&l.x>marker.x);
        const shaped=body&&layout.runs.find(r=>r.start===body.start&&r.end===body.end);
        context.textBaseline=shaped?'alphabetic':'top';
        context.fillStyle=marker.color;context.fillText(marker.text,x(marker.x),shaped?y(shaped.baseline):top);
    }
    context.textBaseline='top';
    const runs=layout.runs.filter(r=>r.end<=chars.length);
    const shapedRanges=new Set(runs.map(r=>`${r.start},${r.end}`));
    const selectedStart=Math.min(caret,anchor),selectedEnd=Math.max(caret,anchor);
    for(const cluster of layout.clusters) {
        if(cluster.end>chars.length)continue;
        const highlight=layout.styles.findLast(s=>s.kind==='h'&&s.start<cluster.end&&s.end>cluster.start)?.value;
        if(highlight&&highlight!=='none'){context.fillStyle='#'+highlight;context.fillRect(x(cluster.x0),y(cluster.y0),(cluster.x1-cluster.x0)*scale,(cluster.y1-cluster.y0)*scale);}
        if(cluster.start<selectedEnd&&cluster.end>selectedStart){context.fillStyle='#b8d6ff';context.fillRect(x(cluster.x0),y(cluster.y0),(cluster.x1-cluster.x0)*scale,(cluster.y1-cluster.y0)*scale);}
    }
    context.textBaseline='alphabetic';context.fontKerning='normal';context.textRendering='geometricPrecision';
    for(const run of runs) {
        if(y(run.baseline)+run.height*scale<rect.y||y(run.baseline)-run.height*scale>rect.y+rect.height)continue;
        const has=kind=>layout.styles.some(s=>s.kind===kind&&s.start<=run.start&&s.end>run.start);
        context.font=officeCanvasFont(officeFace(run.face),run.size*scale,has('b'));
        context.direction=run.rtl?'rtl':'ltr';context.textAlign=run.rtl?'right':'left';context.fillStyle=run.color;
        context.fillText(chars.slice(run.start,run.end).join(''),x(run.x+(run.rtl?run.advance:0)),y(run.baseline));
        if(has('u'))context.fillRect(x(run.x),y(run.baseline+run.size*.1),run.advance*scale,Math.max(1,scale));
        if(has('s'))context.fillRect(x(run.x),y(run.baseline-run.size*.3),run.advance*scale,Math.max(1,scale));
    }
    const stop=layout.stops.find(s=>s.offset===caret&&s.affinity===layout.caretAffinity)||layout.stops.find(s=>s.offset===caret);
    if(command.caret&&stop&&caret<=chars.length){context.fillStyle='#125ac4';context.fillRect(x(stop.x),y(stop.y0),Math.max(1,scale*2),(stop.y1-stop.y0)*scale);}
    context.direction='ltr';context.textAlign='left';context.textBaseline='top';
    if(presentation.lineNumbers) {
        context.fillStyle='#6b7280';context.textAlign='right';
        context.font=officeCanvasFont(officeFace(layout.runs[0]?.face??0),Math.max(6,10*scale));
        const seen=new Set();
        for(const line of layout.lines) {
            if(seen.has(line.y))continue;seen.add(line.y);
            const number=(presentation.lineNumberStart||1)+chars.slice(0,line.start).filter(c=>c==='\n').length;
            context.fillText(String(number),x(Math.max(10,line.x-16)),y(line.y));
        }
        context.textAlign='left';
    }
    for(const line of layout.lines) {
        if(shapedRanges.has(`${line.start},${line.end}`))continue;
        const advance=Math.max(1,Math.floor(line.size*3/5))*scale,top=y(line.y),height=line.size*1.5*scale;
        if(top+height<rect.y||top>rect.y+rect.height)continue;
        for(let index=line.start;index<line.end;index++) {
            const left=x(line.x)+(index-line.start)*advance;
            const styles=new Set(layout.styles.filter(s=>s.start<=index&&s.end>index).map(s=>s.kind));
            const highlight=layout.styles.findLast(s=>s.kind==='h'&&s.start<=index&&s.end>index)?.value;
            if(highlight&&highlight!=='none'){context.fillStyle='#'+highlight;context.fillRect(left,top,advance,height);}
            if(index>=Math.min(caret,anchor)&&index<Math.max(caret,anchor)) {context.fillStyle='#b8d6ff';context.fillRect(left,top,advance,height);}
            const font=line.font.replace(/[\x00-\x1f"\\]/g,'');
            context.font=`${styles.has('i')?'italic ':''}${styles.has('b')?700:400} ${line.size*scale}px "${font||'RiceWM Sans'}", "RiceWM CJK", sans-serif`;
            context.fillStyle=line.color;context.fillText(chars[index]||'',left,top,advance*1.3);
            if(styles.has('u'))context.fillRect(left,top+line.size*scale,advance,Math.max(1,scale));
            if(styles.has('s'))context.fillRect(left,top+line.size*.6*scale,advance,Math.max(1,scale));
        }
        if(command.caret&&caret>=line.start&&caret<=line.end) {context.fillStyle='#125ac4';context.fillRect(x(line.x)+(caret-line.start)*advance,top,Math.max(1,scale*2),height);}
    }
    context.restore();return true;
}

// Character geometry is mirrored by rich_attribute_hit in the native input
// boundary. Document pixels are scaled only for viewing; formatting is unchanged.
export function richAttributeLayout(presentation,width) {
    if(!presentation.language.startsWith('rich')||parseOfficeLayout(presentation.language))return null;
    const chars=Array.from(presentation.document),ranges=[];
    for(const token of presentation.language.split('~').slice(0,4096)) {
        const m=/^([fghbiusBNQACRJ])(\d+)-(\d+)(?:,([\da-f]+|none))?$/i.exec(token);
        if(!m)continue;
        if(m[1]==='f'&&(!/^\d+$/.test(m[4])||Number(m[4])<6||Number(m[4])>128))continue;
        if('gh'.includes(m[1])&&!/^[a-f0-9]{6}$/.test(m[4])&&!(m[1]==='h'&&m[4]==='none'))continue;
        if(Number(m[2])>Number(m[3])||Number(m[3])>chars.length)continue;
        ranges.push({kind:m[1],start:Number(m[2]),end:Number(m[3]),value:m[4]});
    }
    const zoom=(presentation.fontSize||12)/12,gutter=presentation.lineNumbers?(String((presentation.lineNumberStart||1)+chars.filter(c=>c==='\n').length).length+1)*8+12:0,available=Math.max(1,(width-8)/zoom-gutter),rows=[];
    const active=(at)=>ranges.filter(r=>r.start<=at&&(r.end>at||('BNQACRJ'.includes(r.kind)&&r.end===at)));
    let y=0,start=0,number=0,line=presentation.lineNumberStart||1;
    while(start<=chars.length) {
        let end=start;while(end<chars.length&&chars[end]!=='\n')end++;
        const block=active(start),list=block.findLast(r=>'BNQ'.includes(r.kind))?.kind;
        number=list==='N'?number+1:0;const marker=list==='B'?'•':list==='N'?number+'.':list==='Q'?'❝':'';
        const indent=marker?28:0,align=block.findLast(r=>'ACRJ'.includes(r.kind))?.kind;
        let at=start,first=true;
        do {
            const from=at;let x=indent,height=18,glyphs=[];
            while(at<end) {
                const style=active(at),sizeRange=style.findLast(r=>r.kind==='f'),size=sizeRange?Math.max(6,Math.min(128,Number(sizeRange.value))):12;
                const advance=editorGlyphAdvance(chars[at],size);
                if(glyphs.length&&presentation.wrap!=='none'&&x+advance>available){
                    if(presentation.wrap==='word'){
                        const space=glyphs.findLastIndex(g=>/\s/u.test(chars[g.at]));
                        if(space>=0){at=glyphs[space].at+1;glyphs= glyphs.slice(0,space+1);x=glyphs.at(-1).x+glyphs.at(-1).advance;height=Math.max(18,...glyphs.map(g=>Math.ceil(g.size*1.5)));}
                    }
                    break;
                }
                glyphs.push({at,x,y,size,advance,style});x+=advance;height=Math.max(height,Math.ceil(size*1.5));at++;
            }
            if(align==='J'&&at<end){
                const last=glyphs.findLastIndex(g=>! /\s/u.test(chars[g.at]));
                const spaces=glyphs.filter((g,i)=>i<last&&chars[g.at]===' ').length;
                if(spaces){const extra=Math.max(0,available-glyphs[last].x-glyphs[last].advance)/spaces;let shift=0;
                    glyphs.forEach((g,i)=>{g.x+=shift;if(i<last&&chars[g.at]===' '){g.advance+=extra;shift+=extra;}});}
            }
            const offset=align==='C'?Math.max(0,(available-x)/2):align==='R'?Math.max(0,available-x):0;
            for(const glyph of glyphs)glyph.x+=offset;
            rows.push({line:first?line:null,start:from,end:at,y,height,glyphs,marker:first?marker:'',x:indent+offset});y+=height;first=false;
        }while(at<end);
        if(end===chars.length)break;start=end+1;line++;
    }
    return {chars,rows,zoom,gutter};
}
export function paintAttributedRichEditor(context,command,presentation) {
    const layout=richAttributeLayout(presentation,command.rect.width);if(!layout)return false;
    const {chars,rows,zoom,gutter}=layout,{rect}=command,caret=presentation.caret??0,anchor=presentation.anchor??caret;
    const row=rows.find((r,i)=>caret>=r.start&&(caret<r.end||(caret===r.end&&(i===rows.length-1||rows[i+1].start>caret))))||rows.at(-1);
    const scroll=Math.max(0,row.y+row.height-(rect.height-8)/zoom);
    const x=n=>rect.x+4+(gutter+n)*zoom,y=n=>rect.y+4+(n-scroll)*zoom;
    const color=command.color||[32,33,36,255],ink=`rgba(${color[0]},${color[1]},${color[2]},${(color[3]??255)/255})`;
    context.save();context.beginPath();context.rect(rect.x,rect.y,rect.width,rect.height);context.clip();context.textBaseline='alphabetic';context.textAlign='left';context.fontKerning='none';
    for(const r of rows) {
        if(y(r.y)+r.height*zoom<rect.y||y(r.y)>rect.y+rect.height)continue;
        if(presentation.lineNumbers&&r.line!==null){context.font='400 12px "RiceWM Mono", "RiceWM CJK", monospace';context.fillStyle=ink;context.save();context.translate(rect.x+4,y(r.y+r.height/1.5));context.scale(zoom,zoom);context.fillText(String(r.line),0,0);context.restore();}
        if(r.marker){context.font=`12px "RiceWM Sans",sans-serif`;context.fillStyle=ink;context.save();context.translate(x(0),y(r.y+r.height/1.5));context.scale(zoom,zoom);context.fillText(r.marker,0,0);context.restore();}
        for(const g of r.glyphs) {
            const has=k=>g.style.some(s=>s.kind===k),value=k=>g.style.findLast(s=>s.kind===k)?.value;
            const foreground=value('g'),highlight=value('h');
            if(highlight&&highlight!=='none'){context.fillStyle='#'+highlight.padStart(6,'0');context.fillRect(x(g.x),y(g.y),g.advance*zoom,r.height*zoom);}
            if(g.at>=Math.min(caret,anchor)&&g.at<Math.max(caret,anchor)){context.fillStyle='#b8d6ff';context.fillRect(x(g.x),y(g.y),g.advance*zoom,r.height*zoom);}
            context.fillStyle=foreground?'#'+foreground.padStart(6,'0'):ink;
            context.font=`${has('i')?'italic ':''}${has('b')?700:400} ${g.size}px "RiceWM Sans", "RiceWM CJK", sans-serif`;
            const textY=g.y+r.height/1.5;
            context.save();context.translate(x(g.x),y(textY));context.scale(zoom,zoom);context.fillText(chars[g.at],0,0);if(has('b'))context.fillText(chars[g.at],Math.max(.6,Math.min(1,g.size/16)),0);context.restore();
            if(has('u'))context.fillRect(x(g.x),y(textY+g.size*.1),g.advance*zoom,Math.max(1,zoom));
            if(has('s'))context.fillRect(x(g.x),y(textY-g.size*.3),g.advance*zoom,Math.max(1,zoom));
        }
    }
    if(command.caret&&row){const g=row.glyphs.find(g=>g.at===caret),last=row.glyphs.at(-1),cx=g?.x??(last?last.x+last.advance:row.x),size=g?.size??last?.size??12;context.fillStyle=ink;context.fillRect(x(cx),y(row.y+row.height/1.5-size),Math.max(1,zoom),size*1.2*zoom);}
    context.restore();return true;
}
