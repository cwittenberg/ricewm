import { createCancellationScope, SceneBuilder, openTypographyFontBank, translateShapedTextLayout, composeShapedTextLayouts,paragraphListLabels } from '@ricewm/app';
import { sourceParagraphs, paragraphSpace } from './paragraph-layout.mjs';
import { textBox, horizontalOffset, verticalOffset } from './text-box-layout.mjs';

const CACHE_BYTES = 8 * 1024 * 1024;
export const MAX_TEXT_FRAME_BYTES = 768 * 1024; // Leaves native frame space for geometry/chrome.
const PATHS_PER_OBJECT = 8192, MAX_IDENTITIES = 8192, MAX_GLYPHS = 2048;
const STYLE_FALLBACK = 'Some text is shown in regular weight/style because the requested font face is unavailable.';
const clone = value => structuredClone(value);
const encoder = new TextEncoder();
function packedColor(value, opacity = 1) {
    if (!Number.isFinite(opacity) || opacity < 0 || opacity > 1) throw new TypeError('Invalid text opacity');
    if (!/^#[0-9a-f]{6}$/iu.test(value)) throw new TypeError('Invalid text color');
    const rgb = Number.parseInt(value.slice(1), 16);
    return ((Math.round(opacity * 255) << 24) | ((rgb & 255) << 16) | (rgb & 0xff00) | (rgb >>> 16)) >>> 0;
}
function textRequest(object, box, notices) {
    const source = object.text, style = source.style || {};
    if (source.mixed && !source.runs?.length) notices.push('Mixed formatting is unavailable because the source run information is missing.');
    if (source.unsupported?.some(value=>value!=='MIXED_TEXT_STYLE'&&value!=='UNRESOLVED_TEXT_STYLE')) notices.push('Some text formatting is not supported yet.');
    const defaults = [];
    const value = (name, fallback) => {
        if (style[name] === null || style[name] === undefined) { defaults.push(name); return fallback; }
        return style[name];
    };
    const request = {
        text: source.text, families: value('fontFamily', 'Liberation Sans'), size: value('fontSize', 20),
        bold: value('bold', false), italic: value('italic', false), alignment: value('align', 'left'),
        color: packedColor(value('color', '#000000'), style.opacity ?? 1), width: box.wrap ? box.width : null,
    };
    const underline = value('underline', false);
    if (defaults.length) notices.push('Some text uses preview formatting because its inherited style is not resolved.');
    if (typeof request.text !== 'string' || encoder.encode(request.text).length > 16384 ||
        typeof request.families !== 'string' || request.families.length > 4096 ||
        !Number.isFinite(request.size) || request.size <= 0 || request.size > 4096 ||
        (request.width !== null && (!Number.isFinite(request.width) || request.width <= 0 || request.width > 1_000_000)) ||
        !['left','center','right','start','end'].includes(request.alignment) ||
        [request.bold,request.italic,underline].some(value=>typeof value !== 'boolean')) throw new TypeError('Unsupported bounded text style');
    if (/^['"]?Arial['"]?$/iu.test(request.families.trim())) {
        request.families = 'Liberation Sans';
        notices.push('Arial is previewed with the bundled Liberation Sans font.');
    }
    return {request,underline};
}
function richParagraphs(object, box, notices) {
    const runs = object.text.runs?.length ? object.text.runs : box.wrap && !object.text.paragraphs?.length ? null : [{ text: object.text.text, style: object.text.style }];
    if (!runs?.length) return null;
    if (!Array.isArray(runs) || runs.length > 4096) throw new RangeError('Source text run budget exceeded');
    const resolved=[];let combined='',sourceBytes=0;
    for(const run of runs){
        if(typeof run.text!=='string')throw new TypeError('Invalid source text run');
        combined+=run.text;
        if((sourceBytes+=encoder.encode(run.text).length)>16384)throw new RangeError('Source text byte budget exceeded');
        const {request,underline}=textRequest({...object,text:{text:run.text,style:run.style}},box,notices);
        const {text,width,...style}=request;style.underline=underline;
        const key=JSON.stringify(style),previous=resolved.at(-1);
        if(previous?.key===key)previous.text+=text;else resolved.push({text,style,key});
    }
    if(combined!==object.text.text)throw new TypeError('Source text runs do not cover the displayed text');
    if(resolved.length>256)throw new RangeError('Resolved text style span budget exceeded');
    if (object.text.paragraphs?.length) return sourceParagraphs(object.text, resolved, style => {
        const { request, underline } = textRequest({ text: { text: '', style } }, box, notices);
        const { text, width, ...paint } = request; return { ...paint, underline };
    });
    const paragraphs=[];let offset=0,current={text:'',spans:[],style:resolved[0].style,offset:0};
    for(const run of resolved){
        const parts=run.text.split('\n');
        for(let index=0;index<parts.length;index++){
            const text=parts[index],bytes=encoder.encode(text).length;
            if(text){
                if(!current.text)current.style=run.style;
                else if(run.style.alignment!==current.style.alignment)notices.push('Some paragraph alignment is inconsistent and uses its first text setting.');
                const start=encoder.encode(current.text).length;
                const {alignment,...paint}=run.style;
                current.spans.push({start,end:start+bytes,...paint});current.text+=text;offset+=bytes;
            }
            if(index+1<parts.length){paragraphs.push(current);offset++;current={text:'',spans:[],style:run.style,offset};}
        }
    }
    paragraphs.push(current);
    if(paragraphs.length>256)throw new RangeError('Text paragraph budget exceeded');
    return paragraphs;
}
async function shapeWithFallback(backend, request, notices) {
    let candidate=request;const styleFallbacks=[];
    for(let attempt=0;attempt<=32;attempt++){
        try{return {...await backend.shape(candidate),styleFallbacks};}
        catch(error){
            if(error.code!=='FONT_STYLE_UNAVAILABLE'&&error.message!=='Typography: UnsupportedStyle')throw error;
            const index=error.styleIndex||0,target=index?candidate.spans?.[index-1]:candidate;
            if(!target||(!target.bold&&!target.italic))throw error;
            if(attempt===32)throw new RangeError('Font style fallback budget exceeded');
            notices.push(STYLE_FALLBACK);styleFallbacks.push({styleIndex:index,fontFamily:target.families,bold:target.bold,italic:target.italic});
            candidate={...candidate};
            if(index)candidate.spans=candidate.spans.map(span=>span.families===target.families&&span.bold===target.bold&&span.italic===target.italic?{...span,bold:false,italic:false}:span);
            else {candidate.bold=false;candidate.italic=false;}
        }
    }
}
function fontNotices(backend,frame,request,notices){
    const fallback=new Set();
    for(const glyph of frame.glyphs){
        const actual=(frame.fontFaces??backend.fonts)[glyph.fontSlot]?.family;
        const requested=(request.spans?.[glyph.styleIndex-1]?.families||request.families).replace(/['"]/gu,'').split(',')[0].trim();
        if(actual&&actual!==requested)fallback.add(actual);
    }
    if(fallback.size)notices.push(`Some characters use available fallback fonts: ${[...fallback].join(', ')}.`);
}
async function shapeObject(backend,request,paragraphs,box,notices){
    if(!paragraphs){const frame=await shapeWithFallback(backend,request,notices);fontNotices(backend,frame,request,notices);return frame;}
    const builder=new SceneBuilder(1,1),glyphs=[],styleFallbacks=[],fontFaces=[],fontIndices=new Map(),editingParts=[],listMarkers=[],labels=paragraphListLabels(paragraphs);let width=0,height=0,lines=0,pathCount=0,bytes=0,snapshot=0,markerGlyphs=0;
    for(const[index,paragraph]of paragraphs.entries()){
        const format=paragraph.paragraphFormat,left=format?.marginLeft??0,right=format?.marginRight??0,contentWidth=box.width-left-right;
        if(!Number.isFinite(contentWidth)||contentWidth<=0)throw RangeError('Paragraph margins exceed text box width');
        const query={...request,...paragraph.style,text:paragraph.text,spans:paragraph.spans,width:box.wrap?contentWidth:null,lineHeight:paragraph.lineHeight?.kind === 'normal' ? null : paragraph.lineHeight ?? null,pathBase:request.pathBase+pathCount,editing:request.editing||Boolean(labels[index])};
        const frame=await shapeWithFallback(backend,query,notices);
        if(glyphs.length+frame.glyphCount>MAX_GLYPHS||pathCount+frame.pathCount>=PATHS_PER_OBJECT)throw new RangeError('Text object exceeds glyph budget');
        bytes+=frame.displayList.length+40;if(bytes>MAX_TEXT_FRAME_BYTES)throw new RangeError('Text object exceeds frame budget');
        fontNotices(backend,frame,query,notices);
        styleFallbacks.push(...frame.styleFallbacks.map(value=>({...value,paragraphOffset:paragraph.offset})));
        const before = paragraphSpace(paragraph.spaceBefore, frame.firstLineFontSize);
        const after = paragraphSpace(paragraph.spaceAfter, frame.lastLineFontSize);
        height += before;
        if (!Number.isFinite(height + frame.height + after) || height + frame.height + after > 1_000_000) throw new RangeError('Paragraph height budget exceeded');
        const x=left+horizontalOffset({...box,width:contentWidth},frame,query.alignment);
        builder.pushTransform([1,0,0,1,x,height]).append(frame.displayList).popTransform();
        if(labels[index]){
            // Markers have native font outlines, but no source character,
            // caret, copied text or editable offset. Keep their geometry apart.
            const properties=format.marker||{},markerColor=properties.resolvedColor??(properties.color?.startsWith('#')?{color:properties.color,opacity:1}:null);
            const markerRequest={...query,text:labels[index],spans:[],width:null,lineHeight:null,alignment:'left',editing:true,
                size:properties.sizePoints===undefined?query.size*(properties.sizePercent??100)/100:properties.sizePoints*96/72,
                families:properties.fontFamily&&!properties.fontFamily.startsWith('+')?properties.fontFamily:query.families,
                color:markerColor?packedColor(markerColor.color,markerColor.opacity):query.color,pathBase:request.pathBase+pathCount+frame.pathCount};
            if(/^Arial$/iu.test(markerRequest.families))markerRequest.families='Liberation Sans';
            let marker;
            try{marker=await shapeWithFallback(backend,markerRequest,notices);}
            catch(error){if(!/MissingGlyph/u.test(String(error.message)))throw error;notices.push('The bullet character is unavailable in the installed fonts. Its source is preserved.');}
            if(marker){
            const baseline=frame.editing.lines[0].baseline,markerY=height+baseline-marker.editing.lines[0].baseline;
            const markerX=Math.max(0,left+(format.indent||0));
            pathCount+=marker.pathCount;markerGlyphs+=marker.glyphCount;bytes+=marker.displayList.length+40;
            if(glyphs.length+frame.glyphCount+markerGlyphs>MAX_GLYPHS||pathCount+frame.pathCount>=PATHS_PER_OBJECT||bytes>MAX_TEXT_FRAME_BYTES)throw RangeError('Paragraph list marker budget exceeded');
            builder.pushTransform([1,0,0,1,markerX,markerY]).append(marker.displayList).popTransform();
            listMarkers.push({text:labels[index],paragraphOffset:paragraph.offset,x:markerX,y:markerY,width:marker.width,height:marker.height,baseline:height+baseline,fontSize:markerRequest.size,color:markerRequest.color,glyphIds:marker.glyphs.map(g=>g.glyphId)});
            }
        }
        const indices=(frame.fontFaces??backend.fonts).map(face=>{
            const key=face.resource?'sdk:'+face.resource:'package:'+face.path;
            if(!fontIndices.has(key)){if(fontFaces.length>=128)throw new RangeError('Text object font inventory limit');fontIndices.set(key,fontFaces.length);fontFaces.push(face);}
            return fontIndices.get(key);
        });
        glyphs.push(...frame.glyphs.map(glyph=>({...glyph,fontSlot:indices[glyph.fontSlot],x:glyph.x+x,y:glyph.y+height,carets:glyph.carets.map(value=>value+x),cluster:glyph.cluster.map(value=>value+paragraph.offset)})));
        if(request.editing)editingParts.push({text:paragraph.text,offset:paragraph.offset,x,y:height,layout:frame.editing});
        width=Math.max(width,left+frame.width+right);height+=frame.height+after;lines+=frame.lines;pathCount+=frame.pathCount;snapshot=frame.snapshot;
    }
    const displayList=builder.finish().displayList;
    let editing;
    if(request.editing) {
        if(editingParts.length===1)editing=translateShapedTextLayout(request.text,editingParts[0].layout,editingParts[0].x,editingParts[0].y);
        else editing=composeShapedTextLayouts(request.text,editingParts);
    }
    return {snapshot,width,height,lines,glyphCount:glyphs.length+markerGlyphs,frameBytes:displayList.length,pathCount,caretModel:'cluster-edges',displayList,glyphs,listMarkers,styleFallbacks,fontFaces,...(editing?{editing}:{})};
}

function positionFrame(frame, box) {
    const x=box.x,y=verticalOffset(box,frame);
    const displayList=new SceneBuilder(1,1).pushTransform([1,0,0,1,x,y]).append(frame.displayList).popTransform().finish().displayList;
    return {...frame,displayList,frameBytes:displayList.length,textOffset:[x,y],contentBox:{...box},
        glyphs:frame.glyphs.map(glyph=>({...glyph,x:glyph.x+x,y:glyph.y+y,carets:glyph.carets.map(value=>value+x)})),
        ...(frame.editing?{editing:translateShapedTextLayout(frame.sourceText,frame.editing,x,y)}:{})};
}

/** One font worker and bounded derived cache per Deck window. Source XML,
 * edits and document revisions remain owned by the document engine. */
export function createDeckTextLayout({services,signal,sharedFonts} = {}) {
    const lifetime = createCancellationScope();
    let closed = false, opening = null, engine = null, busy = false, cacheBytes = 0, closing = null, openingFailed = false;
    const cache = new Map(), identities = new Map();
    const alive = () => { if (closed || signal?.aborted) throw new Error('Text layout closed'); };
    async function worker() {
        alive();
        opening ||= openTypographyFontBank({services,signal:lifetime.signal,...(sharedFonts===undefined?{}:{sharedFonts})})
            .then(value=>{engine=value;return value;}).catch(error=>{
                // A temporary asset failure must not poison every later slide.
                // The font bank cleans up its failed initialization itself.
                opening=null;openingFailed=true;throw error;
            });
        const value = await opening;alive();return value;
    }
    function close() {
        if (closing) return closing;
        closed = true; signal?.removeEventListener('abort', abort);lifetime.abort();
        cache.clear();identities.clear();cacheBytes=0;
        closing = (async () => {
            try { await (engine || await opening)?.close(); } catch { /* Failed initialization owns its cleanup. */ }
        })();
        return closing;
    }
    function abort(){void close();}
    signal?.addEventListener('abort',abort,{once:true});
    function remember(key,frame,notices) {
        const entry = {frame:clone(frame),notices:[...notices]};
        // Bound metadata, keys and vectors, not just the path payload.
        entry.bytes = frame.displayList.byteLength + 2 * (JSON.stringify({...frame,displayList:null}).length + key.length + notices.join('').length) + frame.glyphs.length * 256;
        if (entry.bytes > CACHE_BYTES) return;
        while(cacheBytes + entry.bytes > CACHE_BYTES && cache.size){const first=cache.keys().next().value;cacheBytes-=cache.get(first).bytes;cache.delete(first);}
        cache.set(key,entry);cacheBytes+=entry.bytes;
    }
    return Object.freeze({
        async render(scene, { placeholderPrompts = false, editing = false } = {}) {
            if(typeof editing!=='boolean')throw TypeError('Text editing geometry requires a boolean');
            alive();if(busy)throw new Error('Text layout render already pending');
            if(!scene || typeof scene.slideId!=='string' || scene.slideId.length>4096 || !Array.isArray(scene.objects) || scene.objects.length>8192)throw new TypeError('Invalid bounded text scene');
            // Capture source-facing inputs before awaiting the font worker.
            const slideId=scene.slideId, seen=new Set(),preferred=[scene.designTheme?.majorFont,scene.designTheme?.minorFont].filter(value=>typeof value==='string'&&value.trim());
            const objects=scene.objects.filter(object=>object?.text?.text || editing&&object?.text || placeholderPrompts && object?.text && object.placeholder?.prompt).map(object=>{
                if(seen.has(object.id))throw new TypeError('Duplicate text object ID');seen.add(object.id);
                const placeholder = !object.text.text && placeholderPrompts && object.placeholder?.prompt;
                const captured = {id:object.id,placeholder:Boolean(placeholder),bounds:object.bounds?{...object.bounds}:null,text:{text:placeholder || object.text.text,
                    paragraphs:object.text.paragraphs ? clone(object.text.paragraphs) : undefined,layout:object.text.layout?{...object.text.layout}:null,style:{...object.text.style},mixed:object.text.mixed,unsupported:object.text.unsupported?.slice(0,128),runs:Array.isArray(object.text.runs)?object.text.runs.slice(0,4097).map(run=>({text:run.text,style:{...run.style}})):null}};
                if (placeholder) { captured.text.style.color = '#777777'; captured.text.mixed = false; captured.text.runs = []; captured.text.paragraphs = undefined; }
                return captured;
            });
            const frames=new Map(),notices=[],diagnostics=[];let total=0,editingBytes=0;busy=true;openingFailed=false;
            try {
                for(const object of objects){
                    alive();
                    const id=object.id;
                    try {
                        if(typeof id!=='string'||!id||id.length>4096||!object.bounds)throw new TypeError('Text geometry unavailable');
                        const box=textBox(object,notices);
                        const {request,underline}=textRequest(object,box,notices);request.underline=underline;if(editing)request.editing=true;
                        const paragraphs=richParagraphs(object,box,notices);
                        const identity=JSON.stringify([slideId,id]);
                        if(!identities.has(identity)){
                            if(identities.size>=MAX_IDENTITIES)throw new RangeError('Text object identity budget exhausted');
                            identities.set(identity,1+identities.size*PATHS_PER_OBJECT);
                        }
                        request.pathBase=identities.get(identity);
                        // Vertical anchoring/margins move the existing outlines;
                        // they must not invalidate expensive glyph shaping.
                        const key=JSON.stringify([identity,request,paragraphs,box.wrap?null:box.width]);
                        let frame,additional=[];
                        const cached=cache.get(key);
                        if(cached){cache.delete(key);cache.set(key,cached);frame=clone(cached.frame);additional=cached.notices;}
                        else {
                            const backend=await worker();backend.setPreferredFamilies(preferred);
                            frame=await shapeObject(backend,request,paragraphs,box,additional);alive();
                            if(frame.glyphCount>MAX_GLYPHS||frame.pathCount>=PATHS_PER_OBJECT)throw new RangeError('Text object exceeds glyph budget');
                            remember(key,frame,additional);
                        }
                        if(editing)frame.sourceText=object.text.text;
                        frame=positionFrame(frame,box);
                        if (object.placeholder) frame.placeholder = true;
                        if(total+frame.displayList.length>MAX_TEXT_FRAME_BYTES)throw new RangeError('Slide text exceeds native frame budget');
                        const geometryBytes=frame.editing?encoder.encode(JSON.stringify(frame.editing)).length:0;
                        if(editingBytes+geometryBytes>4*1024*1024)throw RangeError('Slide text editing geometry exceeds its frame budget');
                        editingBytes+=geometryBytes;total+=frame.displayList.length;frames.set(id,frame);notices.push(...additional);
                        if(additional.includes(STYLE_FALLBACK))diagnostics.push({objectId:id,code:'FONT_STYLE_UNAVAILABLE',requestedStyle:paragraphs?'mixed':{bold:request.bold,italic:request.italic},affectedRuns:frame.styleFallbacks,previewStyle:{bold:false,italic:false}});
                        if(frame.height>box.height)notices.push('Some text extends beyond its box height.');
                    }catch(error){
                        alive();
                        const detail=String(error.message||error).slice(0,220);
                        diagnostics.push({objectId:id,message:detail});
                        notices.push(/budget|limit|exceeds/iu.test(detail)
                            ? 'Some text is too complex for the current canvas preview.'
                            : 'Some text could not be rendered with the available fonts and formatting.');
                        // Package/worker initialization is common to all objects;
                        // do not repeatedly start workers for the same failure.
                        if(openingFailed)break;
                    }
                }
                return {frames,notices:[...new Set(notices)].slice(0,256).map(notice=>notice.slice(0,512)),diagnostics:diagnostics.slice(0,256)};
            }finally{busy=false;}
        },
        close,
    });
}
