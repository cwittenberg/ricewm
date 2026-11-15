export const MAX_SHAPED_TEXT_BYTES=16384;
export const MAX_SHAPED_LAYOUT_BYTES=4*1024*1024;
const encoder=new TextEncoder();
const integer=(value,max)=>Number.isSafeInteger(value)&&value>=0&&value<=max;
const number=value=>Number.isFinite(value)&&Math.abs(value)<=1_000_000;
const exact=(value,keys)=>value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).length===keys.length&&keys.every(key=>Object.hasOwn(value,key));
const fail=()=>{throw new RangeError('Invalid shaped text editing map');};

/** The native shaper owns line breaking, bidi topology, ligature continuations
 * and both caret affinities. Consumers must not reconstruct these from glyph
 * advances, browser measurements or Unicode scalar widths. */
export function checkedShapedTextLayout(text,value) {
    if(typeof text!=='string'||encoder.encode(text).length>MAX_SHAPED_TEXT_BYTES||
        !value||value.version!==1||value.textBytes!==encoder.encode(text).length||
        !integer(value.lineCount,MAX_SHAPED_TEXT_BYTES+1)||!integer(value.clusterCount,32768)||!integer(value.stopCount,32770)||
        !integer(value.bytes,MAX_SHAPED_LAYOUT_BYTES)||!Array.isArray(value.lines)||!Array.isArray(value.clusters)||!Array.isArray(value.stops)||
        value.lines.length!==value.lineCount||value.clusters.length!==value.clusterCount||value.stops.length!==value.stopCount)fail();
    if(encoder.encode(JSON.stringify(value)).length>MAX_SHAPED_LAYOUT_BYTES)fail();
    // Parley's empty paragraph has an unprinted terminal position at 1 even
    // though its source and only insertion stop are both at byte 0. Retain its
    // actual metrics/topology and normalize that one sentinel to source space.
    // All other out-of-source line/cluster/stop positions remain invalid.
    if(text===''&&value.lineCount===1&&value.clusterCount===0&&value.stopCount===1&&
        value.lines[0]?.start===0&&value.lines[0]?.end===1&&value.stops[0]?.offset===0)
        value={...value,lines:[{...value.lines[0],end:0}]};
    const boundaries=new Set([0]);let bytes=0;
    for(const character of text){const point=character.codePointAt(0);if(point>=0xd800&&point<=0xdfff)fail();bytes+=encoder.encode(character).length;boundaries.add(bytes);}
    const offset=value=>Number.isSafeInteger(value)&&boundaries.has(value);
    for(const line of value.lines)if(!exact(line,['start','end','break','baseline','ascent','descent','x0','x1','y0','y1'])||
        !offset(line.start)||!offset(line.end)||line.start>line.end||!['none','soft','hard'].includes(line.break)||
        !['baseline','ascent','descent','x0','x1','y0','y1'].every(key=>number(line[key]))||line.x1<line.x0||line.y1<line.y0)fail();
    const lineIndex=index=>integer(index,value.lineCount-1),stopIndex=index=>integer(index,value.stopCount-1);
    for(const stop of value.stops)if(!exact(stop,['offset','affinity','line','x','y0','y1','left','right','wordLeft','wordRight'])||
        !offset(stop.offset)||!['upstream','downstream'].includes(stop.affinity)||!lineIndex(stop.line)||
        !['x','y0','y1'].every(key=>number(stop[key]))||stop.y1<stop.y0||!['left','right','wordLeft','wordRight'].every(key=>stopIndex(stop[key])))fail();
    for(const cluster of value.clusters)if(!exact(cluster,['start','end','line','x0','x1','rtl','wordBoundary','hardBreak','ligatureContinuation','leading','trailing'])||
        !offset(cluster.start)||!offset(cluster.end)||cluster.start>=cluster.end||!lineIndex(cluster.line)||
        !number(cluster.x0)||!number(cluster.x1)||cluster.x1<cluster.x0||
        !['rtl','wordBoundary','hardBreak','ligatureContinuation'].every(key=>typeof cluster[key]==='boolean')||!stopIndex(cluster.leading)||!stopIndex(cluster.trailing))fail();
    return Object.freeze({...value,lines:Object.freeze(value.lines.map(row=>Object.freeze({...row}))),clusters:Object.freeze(value.clusters.map(row=>Object.freeze({...row}))),stops:Object.freeze(value.stops.map(row=>Object.freeze({...row})))});
}

export function translateShapedTextLayout(text,value,x=0,y=0) {
    if(!number(x)||!number(y))throw RangeError('Invalid shaped layout translation');
    const map=checkedShapedTextLayout(text,value);
    return checkedShapedTextLayout(text,{...map,
        lines:map.lines.map(row=>({...row,x0:row.x0+x,x1:row.x1+x,y0:row.y0+y,y1:row.y1+y,baseline:row.baseline+y})),
        clusters:map.clusters.map(row=>({...row,x0:row.x0+x,x1:row.x1+x})),
        stops:map.stops.map(row=>({...row,x:row.x+x,y0:row.y0+y,y1:row.y1+y})),
    });
}

/** Compose independently shaped source paragraphs. Each paragraph keeps its
 * native bidi runs, clusters, affinities and visual/word graph. A source LF joins
 * its actual visual exit to the following paragraph's actual visual entry;
 * shaping the combined string would incorrectly reuse one bidi base for all
 * paragraphs. No glyph advance or Unicode width is reconstructed here. */
export function composeShapedTextLayouts(text,parts) {
    if(typeof text!=='string'||!Array.isArray(parts)||!parts.length||parts.length>256)throw RangeError('Invalid shaped paragraph parts');
    const source=encoder.encode(text),lines=[],clusters=[],stops=[],records=[];
    let next=0;
    for(const[partIndex,part]of parts.entries()) {
        if(!part||part.offset!==next||!number(part.x)||!number(part.y)||typeof part.text!=='string')throw RangeError('Shaped paragraphs do not cover source text');
        const bytes=encoder.encode(part.text),map=translateShapedTextLayout(part.text,part.layout,part.x,part.y);
        if(bytes.length+part.offset>source.length||!bytes.every((value,index)=>value===source[part.offset+index]))throw RangeError('Shaped paragraph text changed');
        const end=part.offset+bytes.length,last=partIndex===parts.length-1;
        if(last?end!==source.length:source[end]!==10)throw RangeError('Shaped paragraph separators changed');
        if(!map.lineCount||!map.stopCount)throw RangeError('Native paragraph has no insertion geometry');
        const lineBase=lines.length,stopBase=stops.length;
        const first=map.stops.findIndex(stop=>stop.offset===0&&stop.affinity==='downstream');
        const entry=first<0&&bytes.length===0?0:first;
        if(entry<0)throw RangeError('Native paragraph has no initial caret');
        const firstStop=map.stops[entry],left=firstStop.left===entry,right=firstStop.right===entry;
        if(bytes.length&&left===right)throw RangeError('Native paragraph direction is ambiguous');
        const rtl=bytes.length>0&&right,forward=rtl?'left':'right',backward=rtl?'right':'left';
        const exits=map.stops.flatMap((stop,index)=>stop.line===map.lineCount-1&&stop[forward]===index?[index]:[]);
        if(!exits.length)throw RangeError('Native paragraph has no visual exit');
        const exit=exits.find(index=>map.stops[index].offset===bytes.length)??exits[0];
        for(const row of map.lines)lines.push({...row,start:row.start+part.offset,end:row.end+part.offset});
        for(const row of map.stops)stops.push({...row,offset:row.offset+part.offset,line:row.line+lineBase,
            left:row.left+stopBase,right:row.right+stopBase,wordLeft:row.wordLeft+stopBase,wordRight:row.wordRight+stopBase});
        for(const row of map.clusters)clusters.push({...row,start:row.start+part.offset,end:row.end+part.offset,line:row.line+lineBase,leading:row.leading+stopBase,trailing:row.trailing+stopBase});
        const record={map,offset:part.offset,end,lineBase,stopBase,entry:entry+stopBase,exit:exit+stopBase,forward,backward,rtl,first:partIndex===0,last};
        // Explicit breaks have an upstream arrival and downstream departure.
        // Their geometry is the native paragraph's visual edge, including bidi
        // runs where that edge and the last logical character differ.
        const boundary=(offset,affinity,nativeIndex)=>{
            const found=map.stops.findIndex(stop=>stop.offset===offset-part.offset&&stop.affinity===affinity);
            if(found>=0)return found+stopBase;
            const row=stops[nativeIndex],index=stops.length;stops.push({...row,offset,affinity});return index;
        };
        record.arrival=record.first?record.entry:boundary(part.offset,'upstream',record.entry);
        record.departure=last?record.exit:boundary(end,'downstream',record.exit);
        records.push(record);next=end+1;
    }
    for(const[index,part]of records.entries()) {
        const previous=records[index-1],following=records[index+1];
        const connect=(row,localIndex)=>{
            for(const direction of ['left','right','wordLeft','wordRight']) {
                const side=direction.startsWith('word')?direction.slice(4).toLowerCase():direction;
                if(part.map.stops[localIndex][direction]!==localIndex)continue;
                if(side===part.forward&&following)row[direction]=following.arrival;
                else if(side===part.backward&&previous)row[direction]=previous.departure;
            }
        };
        part.map.stops.forEach((_,localIndex)=>connect(stops[part.stopBase+localIndex],localIndex));
        if(part.arrival!==part.entry)connect(stops[part.arrival],part.entry-part.stopBase);
        if(part.departure!==part.exit)connect(stops[part.departure],part.exit-part.stopBase);
        if(following) {
            const line=lines[part.lineBase+part.map.lineCount-1],edge=stops[part.departure];
            line.end=part.end+1;line.break='hard';
            clusters.push({start:part.end,end:part.end+1,line:part.lineBase+part.map.lineCount-1,x0:edge.x,x1:edge.x,
                rtl:part.rtl,wordBoundary:true,hardBreak:true,ligatureContinuation:false,leading:part.departure,trailing:following.arrival});
        }
    }
    // Stable source/affinity ordering keeps index identities deterministic;
    // every native edge is remapped, never inferred from scalar positions.
    const order=stops.map((_,index)=>index).sort((a,b)=>stops[a].offset-stops[b].offset||Number(stops[a].affinity==='upstream')-Number(stops[b].affinity==='upstream'));
    const indices=new Map(order.map((index,next)=>[index,next]));
    const result={version:1,textBytes:source.length,lineCount:lines.length,clusterCount:clusters.length,stopCount:stops.length,bytes:0,lines,
        clusters:clusters.sort((a,b)=>a.line-b.line||a.x0-b.x0).map(row=>({...row,leading:indices.get(row.leading),trailing:indices.get(row.trailing)})),
        stops:order.map(index=>{const row=stops[index];return {...row,left:indices.get(row.left),right:indices.get(row.right),wordLeft:indices.get(row.wordLeft),wordRight:indices.get(row.wordRight)};})};
    for(let pass=0;pass<3;pass++)result.bytes=encoder.encode(JSON.stringify(result)).length;
    return checkedShapedTextLayout(text,result);
}

/** Read-only interaction with a source-bound native layout. Stop identities
 * retain affinity; native control scalar offsets are available through caret.
 * Source editing, text ownership and undo remain with the caller. */
export class ShapedTextLayout {
    #byteToScalar;
    #lineStops;
    #lineClusters;
    static controlFactory=Object.freeze({topics:['documents','visual'],constructor:'new ShapedTextLayout(text,nativeEditingMap)',methods:'map; hit(x,y), caret(stop), stopAtScalar(index,{affinity?,line?}), move(stop,"left"|"right"|"up"|"down"|"home"|"end",{word?,column?}), selectionRects(anchorStop,caretStop). Read-only geometry from openTypographyEngine.shape({editing:true}). Native shaper caret topology and affinities remain authoritative; scalar offsets can be converted to source UTF-8 ranges with textRangeFromSelection.'});
    constructor(text,value) {
        this.map=checkedShapedTextLayout(text,value);this.text=text;
        this.#byteToScalar=new Map([[0,0]]);let offset=0,scalar=0;
        for(const character of text){offset+=encoder.encode(character).length;this.#byteToScalar.set(offset,++scalar);}
        this.#lineStops=Array.from({length:this.map.lineCount},()=>[]);
        this.#lineClusters=Array.from({length:this.map.lineCount},()=>[]);
        this.map.stops.forEach((stop,index)=>this.#lineStops[stop.line].push(index));
        this.map.clusters.forEach(cluster=>this.#lineClusters[cluster.line].push(cluster));
        Object.freeze(this);
    }
    caret(index) {
        const stop=this.map.stops[index];if(!Number.isSafeInteger(index)||!stop)throw RangeError('Caret stop is outside the shaped layout');
        return Object.freeze({...stop,index,scalar:this.#byteToScalar.get(stop.offset)});
    }
    stopAtScalar(scalar,{affinity='downstream',line=null}={}) {
        if(!Number.isSafeInteger(scalar)||scalar<0||!['upstream','downstream'].includes(affinity)||line!==null&&!integer(line,this.map.lines.length-1))throw RangeError('Invalid shaped caret position');
        const matches=this.map.stops.flatMap((stop,index)=>this.#byteToScalar.get(stop.offset)===scalar&&(line===null||line===stop.line)?[index]:[]);
        return matches.find(index=>this.map.stops[index].affinity===affinity)??matches[0]??null;
    }
    hit(x,y) {
        if(!number(x)||!number(y))throw RangeError('Invalid shaped pointer coordinate');
        if(!this.map.lines.length)return null;
        const distance=row=>y<row.y0?row.y0-y:y>row.y1?y-row.y1:0;
        let line=0;for(let i=1;i<this.map.lines.length;i++)if(distance(this.map.lines[i])<distance(this.map.lines[line]))line=i;
        const clusters=this.#lineClusters[line].filter(cluster=>cluster.x1>cluster.x0&&!cluster.hardBreak);
        if(clusters.length) {
            const cluster=clusters.find(cluster=>x<cluster.x1)||clusters.at(-1);
            const before=x<(cluster.x0+cluster.x1)/2;
            return before!==cluster.rtl?cluster.leading:cluster.trailing;
        }
        return this.#lineStops[line].reduce((best,index)=>best===null||Math.abs(this.map.stops[index].x-x)<Math.abs(this.map.stops[best].x-x)?index:best,null);
    }
    move(index,direction,{word=false,column=null}={}) {
        const stop=this.caret(index);
        if(typeof word!=='boolean'||column!==null&&!number(column))throw RangeError('Invalid shaped movement');
        if(['left','right'].includes(direction))return this.map.stops[index][word?'word'+direction[0].toUpperCase()+direction.slice(1):direction];
        if(['up','down'].includes(direction)) {
            const next=Math.max(0,Math.min(this.map.lines.length-1,stop.line+(direction==='up'?-1:1)));
            if(next===stop.line)return index;
            const row=this.map.lines[next];return this.hit(column??stop.x,(row.y0+row.y1)/2)??index;
        }
        if(['home','end'].includes(direction)) {
            const indices=this.#lineStops[stop.line],end=direction==='end';
            return indices.reduce((best,i)=>this.map.stops[i].offset*(end?1:-1)>this.map.stops[best].offset*(end?1:-1)?i:best,indices[0])??index;
        }
        throw RangeError('Unknown shaped movement');
    }
    selectionRects(anchor,caret) {
        const a=this.caret(anchor).offset,b=this.caret(caret).offset,start=Math.min(a,b),end=Math.max(a,b),rects=[];
        if(start===end)return rects;
        for(const cluster of this.map.clusters)if(cluster.start<end&&cluster.end>start) {
            const row=this.map.lines[cluster.line],x=cluster.x0,width=Math.max(cluster.hardBreak?1:0,cluster.x1-cluster.x0);
            if(width<=0)continue;
            const previous=rects.at(-1);
            if(previous&&previous.line===cluster.line&&Math.abs(previous.x+previous.width-x)<.01)previous.width=x+width-previous.x;
            else rects.push({line:cluster.line,x,y:row.y0,width,height:Math.max(1,row.y1-row.y0)});
        }
        return rects;
    }
}
